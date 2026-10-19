use serde_json::{Value, json};
use tempfile::tempdir;
use testar_state_model::*;

fn raw_check() -> Value {
    json!({
        "run": "run-1", "name": "Alternative text is present",
        "rule": {"id": "example:alt-present"},
        "target": {"kind": "widget_property", "observation": "first",
            "widget": "image", "property": "ui.alt"},
        "status": "failed", "details": {}
    })
}

fn check(target: CheckTarget) -> CheckResult {
    let mut raw = raw_check();
    raw["target"] = serde_json::to_value(target).unwrap();
    serde_json::from_value(raw).unwrap()
}

fn envelope(id: &str, event: Event) -> EventEnvelope {
    EventEnvelope {
        schema_version: SCHEMA_VERSION,
        id: EventId::new(id).unwrap(),
        event,
    }
}

fn manifest() -> Manifest {
    Manifest {
        schema_version: SCHEMA_VERSION,
        model: ModelId::new("checks").unwrap(),
    }
}

fn seed() -> Vec<EventEnvelope> {
    let mut events = Vec::new();
    for run in ["run-1", "run-2"] {
        events.push(Event::RunStarted(Run {
            id: RunId::new(run).unwrap(),
            producer: "unit-fixture".into(),
            source_run: run.into(),
            properties: PropertyMap::new(),
        }));
    }
    events.push(Event::ActionDefined(ActionDefinition {
        id: ActionId::new("activate").unwrap(),
        kind: "activate".into(),
        properties: PropertyMap::new(),
    }));
    for (id, run, sequence) in [
        ("first", "run-1", 1),
        ("second", "run-1", 2),
        ("other", "run-2", 1),
    ] {
        let mut properties = PropertyMap::new();
        if id == "first" {
            properties.insert("ui.alt".into(), Value::Null);
        }
        events.push(Event::Observed(Observation {
            id: ObservationId::new(id).unwrap(),
            run: RunId::new(run).unwrap(),
            sequence,
            timestamp_micros: sequence,
            state: ConcreteState {
                id: ConcreteStateId::new(format!("snapshot-{id}")).unwrap(),
                properties: PropertyMap::new(),
                widgets: vec![Widget {
                    id: WidgetId::new("image").unwrap(),
                    parent: None,
                    properties,
                }],
                capture: CaptureScope::Complete,
            },
            candidate_scope: CandidateScope::Generated,
            candidates: vec![ActionCandidate {
                action: ActionId::new("activate").unwrap(),
                target: None,
            }],
            artifacts: vec![],
        }));
    }
    events
        .into_iter()
        .enumerate()
        .map(|(index, event)| envelope(&format!("seed-{index}"), event))
        .collect()
}

fn model() -> StateModel {
    let mut model = StateModel::new(manifest()).unwrap();
    for event in seed() {
        model.apply(event).unwrap();
    }
    model
}

#[test]
fn expectations_and_readings_preserve_omission_null_absence_and_unknown() {
    let omitted: CheckResult = serde_json::from_value(raw_check()).unwrap();
    assert_eq!(omitted.expected, ExpectedValue::Unspecified);
    assert_eq!(omitted.actual, None);
    assert_eq!(serde_json::to_value(&omitted).unwrap(), raw_check());
    for actual in [
        json!({"kind": "value", "value": null}),
        json!({"kind": "value", "value": {"nested": [1, true, null]}}),
        json!({"kind": "absent"}),
        json!({"kind": "unknown"}),
    ] {
        let mut raw = raw_check();
        raw["expected"] = Value::Null;
        raw["actual"] = actual;
        raw["rule"]["version"] = json!("1");
        raw["rule"]["parameters"] = json!({"mode": "presence"});
        let result: CheckResult = serde_json::from_value(raw.clone()).unwrap();
        result.validate().unwrap();
        assert_eq!(result.expected, ExpectedValue::Value(Value::Null));
        assert_eq!(serde_json::to_value(result).unwrap(), raw);
    }
}

#[test]
fn malformed_targets_rules_and_readings_are_rejected() {
    let mut malformed = Vec::new();
    for actual in [
        Value::Null,
        json!({"kind":"value"}),
        json!({"kind":"absent","value":null}),
        json!({"kind":"unknown","extra":true}),
    ] {
        let mut raw = raw_check();
        raw["actual"] = actual;
        malformed.push(raw);
    }
    for (field, value) in [
        ("id", json!(" ")),
        ("version", json!("")),
        ("version", Value::Null),
        ("parameters", Value::Null),
        ("parameters", json!({"":true})),
    ] {
        let mut raw = raw_check();
        raw["rule"][field] = value;
        malformed.push(raw);
    }
    for target in [
        json!({"kind":"run","observation":"first"}),
        json!({"kind":"widget_property","observation":"first","widget":"image"}),
        json!({"kind":"state_property","observation":"first","property":""}),
    ] {
        let mut raw = raw_check();
        raw["target"] = target;
        malformed.push(raw);
    }
    for raw in malformed {
        if let Ok(result) = serde_json::from_value::<CheckResult>(raw.clone()) {
            assert!(
                result.validate().is_err(),
                "Accepted invalid result: {raw}"
            );
        }
    }
}

#[test]
fn property_targets_allow_established_absence_without_creating_other_results() {
    let mut model = model();
    assert!(model.checks().is_empty());
    let targets = [
        CheckTarget::StateProperty {
            observation: ObservationId::new("second").unwrap(),
            property: "ui.missing".into(),
        },
        CheckTarget::WidgetProperty {
            observation: ObservationId::new("second").unwrap(),
            widget: WidgetId::new("image").unwrap(),
            property: "ui.alt".into(),
        },
    ];
    for (index, target) in targets.into_iter().enumerate() {
        let mut result = check(target);
        result.actual = Some(CheckReading::Absent {});
        model
            .apply(envelope(&format!("absent-{index}"), Event::Checked(result)))
            .unwrap();
    }
    assert_eq!(model.checks().len(), 2);
    assert_eq!(model.observation_count(), 3);
    assert_eq!(model.link_count(), 0);
}

#[test]
fn invalid_execution_and_sequence_references_leave_the_model_unchanged() {
    let mut model = model();
    model
        .apply(envelope(
            "select-other",
            Event::ActionSelected(Execution {
                id: ExecutionId::new("other-execution").unwrap(),
                run: RunId::new("run-2").unwrap(),
                source: ObservationId::new("other").unwrap(),
                action: ActionId::new("activate").unwrap(),
                target: None,
                parameters: PropertyMap::new(),
            }),
        ))
        .unwrap();
    let count = model.event_count();
    let targets = [
        (
            CheckTarget::Execution {
                execution: ExecutionId::new("missing").unwrap(),
            },
            "target.execution",
        ),
        (
            CheckTarget::Execution {
                execution: ExecutionId::new("other-execution").unwrap(),
            },
            "target.execution",
        ),
        (
            CheckTarget::Sequence {
                from_observation: ObservationId::new("other").unwrap(),
                to_observation: ObservationId::new("second").unwrap(),
            },
            "target.from_observation",
        ),
        (
            CheckTarget::Sequence {
                from_observation: ObservationId::new("first").unwrap(),
                to_observation: ObservationId::new("other").unwrap(),
            },
            "target.to_observation",
        ),
        (
            CheckTarget::Sequence {
                from_observation: ObservationId::new("second").unwrap(),
                to_observation: ObservationId::new("first").unwrap(),
            },
            "target.to_observation",
        ),
    ];
    for (index, (target, expected_field)) in targets.into_iter().enumerate() {
        let error = model
            .apply(envelope(
                &format!("invalid-{index}"),
                Event::Checked(check(target)),
            ))
            .unwrap_err();
        match error {
            Error::InvalidCheckTarget { field, .. } => {
                assert_eq!(field, expected_field)
            }
            other => panic!("Unexpected validation error: {other}"),
        }
        assert_eq!(model.event_count(), count);
        assert!(model.checks().is_empty());
    }
}

#[test]
fn a_single_observation_sequence_accepts_late_results_and_exact_retries() {
    let mut model = model();
    model
        .apply(envelope(
            "finish",
            Event::RunFinished {
                run: RunId::new("run-1").unwrap(),
                reason: "complete".into(),
            },
        ))
        .unwrap();
    let target = CheckTarget::Sequence {
        from_observation: ObservationId::new("first").unwrap(),
        to_observation: ObservationId::new("first").unwrap(),
    };
    let mut result = check(target);
    result.status = CheckStatus::Pending;
    let event = envelope("late-result", Event::Checked(result.clone()));
    assert!(model.apply(event.clone()).unwrap());
    assert!(!model.apply(event).unwrap());
    assert_eq!(model.checks(), &[result]);
}

#[test]
fn repeated_widget_properties_are_attached_to_their_observations() {
    let mut model = model();
    for observation in ["first", "second"] {
        let result = check(CheckTarget::WidgetProperty {
            observation: ObservationId::new(observation).unwrap(),
            widget: WidgetId::new("image").unwrap(),
            property: "ui.alt".into(),
        });
        model
            .apply(envelope(
                &format!("result-{observation}"),
                Event::Checked(result),
            ))
            .unwrap();
    }
    assert_eq!(model.checks().len(), 2);
    assert_ne!(model.checks()[0].target, model.checks()[1].target);
}

#[test]
fn saved_check_results_reopen_with_explicit_null_and_optional_fields_intact() {
    let directory = tempdir().unwrap();
    let mut store = JsonlStore::create(directory.path(), manifest()).unwrap();
    for event in seed() {
        store.record(event).unwrap();
    }
    let mut raw = raw_check();
    raw["expected"] = Value::Null;
    raw["status"] = json!("passed");
    raw["actual"] = json!({"kind":"value","value":null});
    let result: CheckResult = serde_json::from_value(raw.clone()).unwrap();
    let event = envelope("null-result", Event::Checked(result.clone()));
    store.record(event.clone()).unwrap();
    drop(store);
    let mut reopened = JsonlStore::open(directory.path()).unwrap();
    assert_eq!(reopened.model().checks(), &[result]);
    assert_eq!(
        serde_json::to_value(&reopened.model().checks()[0]).unwrap(),
        raw
    );
    assert!(!reopened.record(event).unwrap());
}
