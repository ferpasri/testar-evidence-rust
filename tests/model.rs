use serde_json::json;
use testar_evidence::{
    ActionCandidate, ActionDefinition, ActionId, CandidateScope, CaptureScope,
    CheckResult, CheckRule, CheckStatus, CheckTarget, ConcreteState,
    ConcreteStateId, Error, Event, EventEnvelope, EventId, Execution,
    ExecutionId, ExecutionStatus, ExpectedValue, Manifest, ModelId,
    Observation, ObservationId, PropertyMap, Run, RunId, RunStatus,
    SCHEMA_VERSION, StateModel,
};

fn model() -> StateModel {
    StateModel::new(Manifest {
        schema_version: SCHEMA_VERSION,
        model: ModelId::new("generic-fixture").unwrap(),
    })
    .unwrap()
}

fn envelope(id: &str, event: Event) -> EventEnvelope {
    EventEnvelope {
        schema_version: SCHEMA_VERSION,
        id: EventId::new(id).unwrap(),
        event,
    }
}

fn record(model: &mut StateModel, id: &str, event: Event) {
    assert!(model.apply(envelope(id, event)).unwrap());
}

fn start_run(model: &mut StateModel, id: &str) {
    record(
        model,
        &format!("start-{id}"),
        Event::RunStarted(Run {
            id: RunId::new(id).unwrap(),
            producer: "generic-fixture".into(),
            source_run: id.into(),
            properties: PropertyMap::new(),
        }),
    );
}

fn define_action(model: &mut StateModel, id: &str) {
    record(
        model,
        &format!("define-{id}"),
        Event::ActionDefined(ActionDefinition {
            id: ActionId::new(id).unwrap(),
            kind: id.into(),
            properties: PropertyMap::new(),
        }),
    );
}

fn observation(
    run: &str,
    id: &str,
    sequence: u64,
    state: &str,
    actions: &[&str],
) -> Observation {
    Observation {
        id: ObservationId::new(id).unwrap(),
        run: RunId::new(run).unwrap(),
        sequence,
        timestamp_micros: sequence * 1_000,
        state: ConcreteState {
            id: ConcreteStateId::new(format!("snapshot-{id}")).unwrap(),
            properties: PropertyMap::from([(
                "sut:screen".into(),
                json!(state),
            )]),
            widgets: Vec::new(),
            capture: CaptureScope::Complete,
        },
        candidates: actions
            .iter()
            .map(|action| ActionCandidate {
                action: ActionId::new(*action).unwrap(),
                target: None,
            })
            .collect(),
        candidate_scope: CandidateScope::Generated,
        artifacts: Vec::new(),
    }
}

fn observe(model: &mut StateModel, observation: Observation) {
    let event_id = format!("observe-{}", observation.id);
    record(model, &event_id, Event::Observed(observation));
}

fn execution(run: &str, id: &str, source: &str, action: &str) -> Execution {
    Execution {
        id: ExecutionId::new(id).unwrap(),
        run: RunId::new(run).unwrap(),
        source: ObservationId::new(source).unwrap(),
        action: ActionId::new(action).unwrap(),
        target: None,
        parameters: PropertyMap::new(),
    }
}

fn select(
    model: &mut StateModel,
    run: &str,
    id: &str,
    source: &str,
    action: &str,
) {
    record(
        model,
        &format!("select-{id}"),
        Event::ActionSelected(execution(run, id, source, action)),
    );
}

fn submit(model: &mut StateModel, id: &str, accepted: bool) {
    record(
        model,
        &format!("submit-{id}"),
        Event::ActionSubmitted {
            execution: ExecutionId::new(id).unwrap(),
            accepted,
            error: (!accepted).then(|| "submission rejected".into()),
        },
    );
}

fn perform(
    model: &mut StateModel,
    id: &str,
    source: &str,
    action: &str,
    target: Observation,
) {
    select(model, target.run.as_str(), id, source, action);
    submit(model, id, true);
    let observation = target.id.clone();
    observe(model, target);
    record(
        model,
        &format!("result-{id}"),
        Event::ActionResult {
            execution: ExecutionId::new(id).unwrap(),
            observation,
        },
    );
}

#[test]
fn observations_are_distinct_and_queries_preserve_run_sequence() {
    let mut model = model();
    start_run(&mut model, "run");
    for (id, sequence, screen) in [("z", 1, "A"), ("a", 2, "B"), ("m", 3, "A")]
    {
        observe(&mut model, observation("run", id, sequence, screen, &[]));
    }
    assert_eq!(model.concrete_state_count(), 3);
    assert_eq!(model.observation_count(), 3);
    assert_eq!(model.link_count(), 0);
    assert_eq!(
        model
            .observations_for_run(&RunId::new("run").unwrap())
            .iter()
            .map(|observation| observation.id.as_str())
            .collect::<Vec<_>>(),
        vec!["z", "a", "m"]
    );
    let snapshot = ConcreteStateId::new("snapshot-z").unwrap();
    assert_eq!(model.observations_for_snapshot(&snapshot).len(), 1);
    assert_eq!(
        model.concrete_state(&snapshot).unwrap().properties["sut:screen"],
        json!("A")
    );
    assert_eq!(model.concrete_states().count(), 3);
    assert!(
        model
            .observations_for_run(&RunId::new("missing").unwrap())
            .is_empty()
    );
    assert!(
        model
            .observations_for_snapshot(
                &ConcreteStateId::new("missing").unwrap()
            )
            .is_empty()
    );
}

#[test]
fn identical_snapshots_can_be_reused_without_merging_observations() {
    let mut model = model();
    start_run(&mut model, "run");
    let first = observation("run", "first", 1, "A", &[]);
    observe(&mut model, first.clone());
    let mut revisit = observation("run", "revisit", 2, "A", &[]);
    revisit.state = first.state.clone();
    observe(&mut model, revisit);
    assert_eq!(model.concrete_state_count(), 1);
    assert_eq!(model.observation_count(), 2);
    assert_eq!(model.observations_for_snapshot(&first.state.id).len(), 2);
    let mut conflict = observation("run", "conflict", 3, "B", &[]);
    conflict.state.id = first.state.id.clone();
    let before = model.event_count();
    assert!(matches!(
        model.apply(envelope("bad-snapshot", Event::Observed(conflict))),
        Err(Error::Conflict(_))
    ));
    assert_eq!(model.event_count(), before);
    assert_eq!(model.concrete_state(&first.state.id), Some(&first.state));
}

#[test]
fn repeated_visits_keep_separate_forward_links_and_execution_order() {
    let mut model = model();
    start_run(&mut model, "run");
    define_action(&mut model, "activate");
    observe(
        &mut model,
        observation("run", "first", 1, "A", &["activate"]),
    );
    perform(
        &mut model,
        "z-first",
        "first",
        "activate",
        observation("run", "second", 2, "A", &["activate"]),
    );
    perform(
        &mut model,
        "a-second",
        "second",
        "activate",
        observation("run", "third", 3, "A", &[]),
    );
    assert_eq!(model.concrete_state_count(), 3);
    assert_eq!(model.link_count(), 2);
    assert_eq!(model.execution_count(), 2);
    let links = model.links().collect::<Vec<_>>();
    assert_eq!(links[0].source.as_str(), "first");
    assert_eq!(links[0].target.as_str(), "second");
    assert_eq!(links[1].source.as_str(), "second");
    assert_eq!(links[1].target.as_str(), "third");
    assert_ne!(links[0].execution, links[1].execution);
    assert_eq!(
        model
            .executions_for_run(&RunId::new("run").unwrap())
            .iter()
            .map(|record| record.execution.id.as_str())
            .collect::<Vec<_>>(),
        vec!["z-first", "a-second"]
    );
    assert!(
        model
            .incoming(&ObservationId::new("first").unwrap())
            .is_empty()
    );
    assert!(
        model
            .outgoing(&ObservationId::new("third").unwrap())
            .is_empty()
    );
    assert_eq!(
        model.outgoing(&ObservationId::new("first").unwrap()),
        vec![links[0].clone()]
    );
    assert_eq!(
        model.incoming(&ObservationId::new("third").unwrap()),
        vec![links[1].clone()]
    );
    assert!(
        !model
            .apply(envelope(
                "result-a-second",
                Event::ActionResult {
                    execution: ExecutionId::new("a-second").unwrap(),
                    observation: ObservationId::new("third").unwrap()
                }
            ))
            .unwrap()
    );
    assert_eq!(model.link_count(), 2);
}

#[test]
fn candidates_parameters_and_attempts_remain_attached_to_their_occurrences() {
    let mut model = model();
    start_run(&mut model, "run");
    for action in ["activate", "confirm", "restart"] {
        define_action(&mut model, action);
    }
    observe(
        &mut model,
        observation("run", "first", 1, "A", &["activate", "confirm"]),
    );
    let mut selected = execution("run", "attempt", "first", "confirm");
    selected
        .parameters
        .insert("input.text".into(), json!("retained input"));
    record(
        &mut model,
        "selection",
        Event::ActionSelected(selected.clone()),
    );
    submit(&mut model, "attempt", false);
    observe(
        &mut model,
        observation("run", "second", 2, "A", &["activate", "restart"]),
    );
    assert_eq!(
        model
            .candidates(&ObservationId::new("second").unwrap())
            .unwrap()
            .iter()
            .map(|candidate| candidate.action.as_str())
            .collect::<Vec<_>>(),
        vec!["activate", "restart"]
    );
    assert_eq!(model.execution(&selected.id).unwrap().execution, selected);
    assert_eq!(model.action_count(), 3);
    assert_eq!(model.actions().count(), 3);
    assert!(model.action(&ActionId::new("confirm").unwrap()).is_some());
    assert!(model.action(&ActionId::new("absent").unwrap()).is_none());
    assert_eq!(model.runs().count(), 1);
    assert_eq!(model.link_count(), 0);
}

#[test]
fn results_cannot_point_to_the_source_or_an_earlier_observation() {
    let mut model = model();
    start_run(&mut model, "run");
    define_action(&mut model, "activate");
    observe(&mut model, observation("run", "earlier", 0, "A", &[]));
    observe(
        &mut model,
        observation("run", "source", 1, "A", &["activate"]),
    );
    select(&mut model, "run", "attempt", "source", "activate");
    submit(&mut model, "attempt", true);
    let before = model.event_count();
    for target in ["earlier", "source"] {
        assert!(matches!(
            model.apply(envelope(
                target,
                Event::ActionResult {
                    execution: ExecutionId::new("attempt").unwrap(),
                    observation: ObservationId::new(target).unwrap()
                }
            )),
            Err(Error::InvalidRecord(_))
        ));
    }
    assert_eq!(model.event_count(), before);
    assert_eq!(model.link_count(), 0);
}

#[test]
fn exact_event_replay_is_idempotent_and_conflicts_do_not_mutate_the_model() {
    let mut model = model();
    start_run(&mut model, "run-1");
    let observed = observation("run-1", "first", 0, "main", &[]);
    let original = envelope("first-event", Event::Observed(observed.clone()));
    assert!(model.apply(original.clone()).unwrap());
    let event_count = model.event_count();

    assert!(!model.apply(original).unwrap());
    assert_eq!(model.event_count(), event_count);
    assert_eq!(model.observation_count(), 1);

    let mut changed = observed.clone();
    changed.timestamp_micros += 1;
    let collision = envelope("first-event", Event::Observed(changed));
    assert!(matches!(model.apply(collision), Err(Error::Conflict(_))));

    let invalid = envelope(
        "invalid-event",
        Event::Observed(observation("absent", "missing-run", 1, "other", &[])),
    );
    assert!(matches!(
        model.apply(invalid),
        Err(Error::UnknownReference(_))
    ));
    assert_eq!(model.event_count(), event_count);
    assert_eq!(model.observation(&observed.id), Some(&observed));
    assert_eq!(model.concrete_state_count(), 1);
    assert_eq!(model.link_count(), 0);
}

#[test]
fn initial_observations_and_cross_run_references_never_invent_edges() {
    let mut model = model();
    start_run(&mut model, "run-1");
    start_run(&mut model, "run-2");
    define_action(&mut model, "activate");
    observe(
        &mut model,
        observation("run-1", "first", 0, "source", &["activate"]),
    );
    observe(
        &mut model,
        observation("run-2", "other", 0, "target", &["activate"]),
    );
    select(&mut model, "run-1", "attempt-1", "first", "activate");
    submit(&mut model, "attempt-1", true);
    let event_count = model.event_count();

    let invalid_result = envelope(
        "cross-result",
        Event::ActionResult {
            execution: ExecutionId::new("attempt-1").unwrap(),
            observation: ObservationId::new("other").unwrap(),
        },
    );
    assert!(matches!(
        model.apply(invalid_result),
        Err(Error::InvalidRecord(_))
    ));
    let invalid_selection = envelope(
        "cross-selection",
        Event::ActionSelected(execution(
            "run-2",
            "attempt-2",
            "first",
            "activate",
        )),
    );
    assert!(matches!(
        model.apply(invalid_selection),
        Err(Error::InvalidRecord(_))
    ));

    assert_eq!(model.event_count(), event_count);
    assert_eq!(model.concrete_state_count(), 2);
    assert_eq!(model.link_count(), 0);
    let pending = model
        .execution(&ExecutionId::new("attempt-1").unwrap())
        .unwrap();
    assert_eq!(pending.status, ExecutionStatus::Submitted);
    assert!(pending.result.is_none());
}

#[test]
fn rejected_and_unfinished_actions_preserve_status_without_fabricating_results()
{
    let mut model = model();
    start_run(&mut model, "run-1");
    define_action(&mut model, "activate");
    observe(
        &mut model,
        observation("run-1", "first", 0, "main", &["activate"]),
    );
    select(&mut model, "run-1", "rejected", "first", "activate");
    submit(&mut model, "rejected", false);
    select(&mut model, "run-1", "not-submitted", "first", "activate");
    observe(
        &mut model,
        observation("run-1", "second", 1, "other", &["activate"]),
    );
    let invalid_result = envelope(
        "missing-submission-result",
        Event::ActionResult {
            execution: ExecutionId::new("not-submitted").unwrap(),
            observation: ObservationId::new("second").unwrap(),
        },
    );
    assert!(matches!(
        model.apply(invalid_result),
        Err(Error::Conflict(_))
    ));
    record(
        &mut model,
        "finish-run-1",
        Event::RunFinished {
            run: RunId::new("run-1").unwrap(),
            reason: "interrupted".into(),
        },
    );

    start_run(&mut model, "run-2");
    observe(
        &mut model,
        observation("run-2", "third", 0, "main", &["activate"]),
    );
    select(&mut model, "run-2", "submitted", "third", "activate");
    submit(&mut model, "submitted", true);
    record(
        &mut model,
        "finish-run-2",
        Event::RunFinished {
            run: RunId::new("run-2").unwrap(),
            reason: "driver disconnected".into(),
        },
    );

    assert_eq!(
        model
            .execution(&ExecutionId::new("rejected").unwrap())
            .unwrap()
            .status,
        ExecutionStatus::Rejected
    );
    for id in ["not-submitted", "submitted"] {
        let record = model.execution(&ExecutionId::new(id).unwrap()).unwrap();
        assert_eq!(record.status, ExecutionStatus::Incomplete);
        assert!(record.result.is_none());
    }
    assert!(matches!(
        model.run_status(&RunId::new("run-1").unwrap()),
        Some(RunStatus::Finished { .. })
    ));
    assert_eq!(model.link_count(), 0);
}

#[test]
fn checks_preserve_scope_and_pending_results_without_inferred_passes() {
    let mut model = model();
    start_run(&mut model, "run-1");
    start_run(&mut model, "run-2");
    observe(&mut model, observation("run-1", "first", 0, "main", &[]));
    observe(&mut model, observation("run-2", "other", 0, "main", &[]));
    let pending = CheckResult {
        run: RunId::new("run-1").unwrap(),
        target: CheckTarget::Run {},
        rule: CheckRule {
            id: "example:eventually-completes".into(),
            version: None,
            parameters: None,
        },
        expected: ExpectedValue::Unspecified,
        actual: None,
        name: "operation eventually completes".into(),
        status: CheckStatus::Pending,
        details: PropertyMap::new(),
    };
    record(&mut model, "pending-check", Event::Checked(pending.clone()));
    let failed = CheckResult {
        target: CheckTarget::Observation {
            observation: ObservationId::new("first").unwrap(),
        },
        name: "button is enabled".into(),
        status: CheckStatus::Failed,
        ..pending.clone()
    };
    record(&mut model, "failed-check", Event::Checked(failed.clone()));

    let invalid = CheckResult {
        target: CheckTarget::Observation {
            observation: ObservationId::new("other").unwrap(),
        },
        ..pending.clone()
    };
    assert!(matches!(
        model.apply(envelope("cross-check", Event::Checked(invalid))),
        Err(Error::InvalidCheckTarget {
            field: "target.observation",
            ..
        })
    ));
    assert_eq!(model.checks(), &[pending, failed]);
    assert!(
        !model
            .checks()
            .iter()
            .any(|check| check.status == CheckStatus::Passed)
    );
    assert_eq!(model.concrete_state_count(), 2);
}

#[test]
fn ordering_stale_sources_and_unknown_candidates_are_rejected_without_partial_updates()
 {
    let mut model = model();
    start_run(&mut model, "run-1");
    define_action(&mut model, "activate");
    observe(
        &mut model,
        observation("run-1", "first", 1, "main", &["activate"]),
    );
    observe(
        &mut model,
        observation("run-1", "second", 2, "main", &["activate"]),
    );
    let event_count = model.event_count();

    for (id, sequence) in [("same-sequence", 2), ("earlier-sequence", 0)] {
        let invalid =
            observation("run-1", id, sequence, "unexpected", &["activate"]);
        assert!(matches!(
            model.apply(envelope(id, Event::Observed(invalid))),
            Err(Error::InvalidRecord(_))
        ));
    }
    let stale =
        Event::ActionSelected(execution("run-1", "stale", "first", "activate"));
    assert!(matches!(
        model.apply(envelope("stale-source", stale)),
        Err(Error::InvalidRecord(_))
    ));
    let missing_candidate = Event::ActionSelected(execution(
        "run-1", "unknown", "second", "absent",
    ));
    assert!(matches!(
        model.apply(envelope("unknown-candidate", missing_candidate)),
        Err(Error::UnknownReference(_))
    ));
    let missing_definition = observation(
        "run-1",
        "unknown-definition",
        3,
        "unexpected",
        &["absent"],
    );
    assert!(matches!(
        model.apply(envelope(
            "unknown-definition",
            Event::Observed(missing_definition)
        )),
        Err(Error::UnknownReference(_))
    ));

    assert_eq!(model.event_count(), event_count);
    assert_eq!(model.observation_count(), 2);
    assert_eq!(model.concrete_state_count(), 2);
    assert_eq!(model.link_count(), 0);
    assert!(
        model
            .execution(&ExecutionId::new("stale").unwrap())
            .is_none()
    );
}
