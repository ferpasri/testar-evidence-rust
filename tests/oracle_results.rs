use serde_json::{Value, json};
use tempfile::tempdir;
use testar_evidence::*;

fn raw_check() -> Value {
    json!({"test_run": "parent-run-1", "test_sequence": "run-1", "name": "Alternative text is present",
        "oracle": {"id": "example:alt-present"}, "oracle_target": {"target_type": "widget_property", "observation": "first",
        "widget": "image", "property": "ui.alt"}, "oracle_status": "failed", "oracle_details": {}})
}

fn check(target: OracleTarget) -> OracleResult {
    let mut raw = raw_check();
    raw["oracle_target"] = serde_json::to_value(target).unwrap();
    serde_json::from_value(raw).unwrap()
}

fn envelope(id: &str, event: Event) -> EvidenceEvent {
    EvidenceEvent {
        schema_version: SCHEMA_VERSION,
        id: EventId::new(id).unwrap(),
        event,
    }
}

fn manifest() -> Manifest {
    Manifest {
        schema_version: SCHEMA_VERSION,
        evidence_id: EvidenceId::new("checks").unwrap(),
    }
}

fn seed() -> Vec<EvidenceEvent> {
    let mut events = Vec::new();
    for run in ["run-1", "run-2"] {
        events.push(Event::TestRunStarted(TestRun {
            id: TestRunId::new(format!("parent-{run}")).unwrap(),
            producer: "unit-fixture".into(),
            source_run: run.into(),
            properties: PropertyMap::new(),
        }));
        events.push(Event::TestSequenceStarted(TestSequence {
            id: TestSequenceId::new(run).unwrap(),
            test_run: TestRunId::new(format!("parent-{}", run)).unwrap(),
            sequence_index: 1,
            source_sequence: run.into(),
            properties: PropertyMap::new(),
        }));
    }
    events.push(Event::ActionDescribed(ActionDescription {
        id: ActionDescriptionId::new("activate").unwrap(),
        action_type: "activate".into(),
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
        events.push(Event::ObservationRecorded(Observation {
            id: ObservationId::new(id).unwrap(),
            test_sequence: TestSequenceId::new(run).unwrap(),
            observation_index: sequence,
            timestamp_micros: sequence,
            recorded_state: RecordedState {
                id: RecordedStateId::new(format!("recorded_state-{id}"))
                    .unwrap(),
                properties: PropertyMap::new(),
                widgets: vec![Widget {
                    id: WidgetId::new("image").unwrap(),
                    parent: None,
                    properties,
                }],
                capture: CaptureScope::Complete,
            },
            available_action_scope: AvailableActionScope::Generated,
            available_actions: vec![AvailableAction {
                action_description: ActionDescriptionId::new("activate")
                    .unwrap(),
                target_widget: None,
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

fn evidence() -> ReconstructedEvidence {
    let mut evidence = ReconstructedEvidence::new(manifest()).unwrap();
    for event in seed() {
        evidence.apply(event).unwrap();
    }
    evidence
}

#[test]
fn expectations_and_readings_preserve_omission_null_absence_and_unknown() {
    let omitted: OracleResult = serde_json::from_value(raw_check()).unwrap();
    assert_eq!(omitted.expected_value, ExpectedValue::Unspecified);
    assert_eq!(omitted.observed_value, None);
    assert_eq!(serde_json::to_value(&omitted).unwrap(), raw_check());
    for actual in [
        json!({"value_status":"value", "value": null}),
        json!({"value_status":"value", "value": {"nested": [1, true, null]}}),
        json!({"value_status":"absent"}),
        json!({"value_status":"unknown"}),
    ] {
        let mut raw = raw_check();
        raw["expected_value"] = Value::Null;
        raw["observed_value"] = actual;
        raw["oracle"]["version"] = json!("1");
        raw["oracle"]["parameters"] = json!({"mode": "presence"});
        let result: OracleResult = serde_json::from_value(raw.clone()).unwrap();
        result.validate().unwrap();
        assert_eq!(result.expected_value, ExpectedValue::Value(Value::Null));
        assert_eq!(serde_json::to_value(result).unwrap(), raw);
    }
}

#[test]
fn malformed_targets_rules_and_readings_are_rejected() {
    let mut malformed = Vec::new();
    for actual in [
        Value::Null,
        json!({"value_status":"value"}),
        json!({"value_status":"absent","value":null}),
        json!({"value_status":"unknown","extra":true}),
    ] {
        let mut raw = raw_check();
        raw["observed_value"] = actual;
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
        raw["oracle"][field] = value;
        malformed.push(raw);
    }
    for target in [
        json!({"kind":"test_sequence","observation":"first"}),
        json!({"target_type":"widget_property","observation":"first","widget":"image"}),
        json!({"target_type":"state_property","observation":"first","property":""}),
    ] {
        let mut raw = raw_check();
        raw["oracle_target"] = target;
        malformed.push(raw);
    }
    for raw in malformed {
        if let Ok(result) = serde_json::from_value::<OracleResult>(raw.clone())
        {
            assert!(
                result.validate().is_err(),
                "Accepted invalid result: {raw}"
            );
        }
    }
}

#[test]
fn property_targets_allow_established_absence_without_creating_other_results() {
    let mut evidence = evidence();
    assert!(evidence.oracle_results().is_empty());
    let targets = [
        OracleTarget::StateProperty {
            observation: ObservationId::new("second").unwrap(),
            property: "ui.missing".into(),
        },
        OracleTarget::WidgetProperty {
            observation: ObservationId::new("second").unwrap(),
            widget: WidgetId::new("image").unwrap(),
            property: "ui.alt".into(),
        },
    ];
    for (index, target) in targets.into_iter().enumerate() {
        let mut result = check(target);
        result.observed_value = Some(ObservedValue::Absent {});
        evidence
            .apply(envelope(
                &format!("absent-{index}"),
                Event::OracleResultRecorded(result),
            ))
            .unwrap();
    }
    assert_eq!(evidence.oracle_results().len(), 2);
    assert_eq!(evidence.observation_count(), 3);
    assert_eq!(evidence.recorded_transition_count(), 0);
}

#[test]
fn invalid_execution_and_sequence_references_leave_the_evidence_unchanged() {
    let mut evidence = evidence();
    evidence
        .apply(envelope(
            "select-other",
            Event::ActionSelected(ActionExecution {
                id: ActionExecutionId::new("other-execution").unwrap(),
                test_sequence: TestSequenceId::new("run-2").unwrap(),
                source_observation: ObservationId::new("other").unwrap(),
                action_description: ActionDescriptionId::new("activate")
                    .unwrap(),
                target_widget: None,
                parameters: PropertyMap::new(),
            }),
        ))
        .unwrap();
    let count = evidence.event_count();
    let targets = [
        (
            OracleTarget::ActionExecution {
                action_execution: ActionExecutionId::new("missing").unwrap(),
            },
            "oracle_target.action_execution",
        ),
        (
            OracleTarget::ActionExecution {
                action_execution: ActionExecutionId::new("other-execution")
                    .unwrap(),
            },
            "oracle_target.action_execution",
        ),
        (
            OracleTarget::ObservationRange {
                first_observation: ObservationId::new("other").unwrap(),
                last_observation: ObservationId::new("second").unwrap(),
            },
            "oracle_target.first_observation",
        ),
        (
            OracleTarget::ObservationRange {
                first_observation: ObservationId::new("first").unwrap(),
                last_observation: ObservationId::new("other").unwrap(),
            },
            "oracle_target.last_observation",
        ),
        (
            OracleTarget::ObservationRange {
                first_observation: ObservationId::new("second").unwrap(),
                last_observation: ObservationId::new("first").unwrap(),
            },
            "oracle_target.last_observation",
        ),
    ];
    for (index, (target, expected_field)) in targets.into_iter().enumerate() {
        let error = evidence
            .apply(envelope(
                &format!("invalid-{index}"),
                Event::OracleResultRecorded(check(target)),
            ))
            .unwrap_err();
        match error {
            Error::Validation {
                code: "invalid_oracle_target",
                field,
                ..
            } => {
                assert_eq!(field, expected_field)
            }
            other => panic!("Unexpected validation error: {other}"),
        }
        assert_eq!(evidence.event_count(), count);
        assert!(evidence.oracle_results().is_empty());
    }
}

#[test]
fn a_single_observation_sequence_accepts_late_results_and_exact_retries() {
    let mut evidence = evidence();
    evidence
        .apply(envelope(
            "finish",
            Event::TestSequenceFinished {
                test_sequence: TestSequenceId::new("run-1").unwrap(),
                reason: "complete".into(),
            },
        ))
        .unwrap();
    let target = OracleTarget::ObservationRange {
        first_observation: ObservationId::new("first").unwrap(),
        last_observation: ObservationId::new("first").unwrap(),
    };
    let mut result = check(target);
    result.oracle_status = OracleStatus::Pending;
    let event =
        envelope("late-result", Event::OracleResultRecorded(result.clone()));
    assert!(evidence.apply(event.clone()).unwrap());
    assert!(!evidence.apply(event).unwrap());
    assert_eq!(evidence.oracle_results(), &[result]);
}

#[test]
fn repeated_widget_properties_are_attached_to_their_observations() {
    let mut evidence = evidence();
    for observation in ["first", "second"] {
        let result = check(OracleTarget::WidgetProperty {
            observation: ObservationId::new(observation).unwrap(),
            widget: WidgetId::new("image").unwrap(),
            property: "ui.alt".into(),
        });
        evidence
            .apply(envelope(
                &format!("result-{observation}"),
                Event::OracleResultRecorded(result),
            ))
            .unwrap();
    }
    assert_eq!(evidence.oracle_results().len(), 2);
    assert_ne!(
        evidence.oracle_results()[0].oracle_target,
        evidence.oracle_results()[1].oracle_target
    );
}

#[test]
fn saved_oracle_results_reopen_with_explicit_null_and_optional_fields_intact() {
    let directory = tempdir().unwrap();
    let mut store = JsonlStore::create(directory.path(), manifest()).unwrap();
    for event in seed() {
        store.record(event).unwrap();
    }
    let mut raw = raw_check();
    raw["expected_value"] = Value::Null;
    raw["oracle_status"] = json!("passed");
    raw["observed_value"] = json!({"value_status":"value","value":null});
    let result: OracleResult = serde_json::from_value(raw.clone()).unwrap();
    let event =
        envelope("null-result", Event::OracleResultRecorded(result.clone()));
    store.record(event.clone()).unwrap();
    drop(store);
    let mut reopened = JsonlStore::open(directory.path()).unwrap();
    assert_eq!(reopened.evidence().oracle_results(), &[result]);
    assert_eq!(
        serde_json::to_value(&reopened.evidence().oracle_results()[0]).unwrap(),
        raw
    );
    assert!(!reopened.record(event).unwrap());
}
