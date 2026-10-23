use serde_json::json;
use testar_evidence::*;

fn evidence() -> ReconstructedEvidence {
    ReconstructedEvidence::new(Manifest {
        schema_version: SCHEMA_VERSION,
        evidence_id: EvidenceId::new("generic-fixture").unwrap(),
    })
    .unwrap()
}

fn envelope(id: &str, event: Event) -> EvidenceEvent {
    EvidenceEvent {
        schema_version: SCHEMA_VERSION,
        id: EventId::new(id).unwrap(),
        event,
    }
}

fn record(evidence: &mut ReconstructedEvidence, id: &str, event: Event) {
    assert!(evidence.apply(envelope(id, event)).unwrap());
}

fn start_run(evidence: &mut ReconstructedEvidence, id: &str) {
    record(
        evidence,
        &format!("parent-{id}"),
        Event::TestRunStarted(TestRun {
            id: TestRunId::new(format!("parent-{id}")).unwrap(),
            producer: "generic-fixture".into(),
            source_run: id.into(),
            properties: PropertyMap::new(),
        }),
    );
    record(
        evidence,
        &format!("start-{id}"),
        Event::TestSequenceStarted(TestSequence {
            id: TestSequenceId::new(id).unwrap(),
            test_run: TestRunId::new(format!("parent-{}", id)).unwrap(),
            sequence_index: 1,
            source_sequence: id.into(),
            properties: PropertyMap::new(),
        }),
    );
}

fn define_action(evidence: &mut ReconstructedEvidence, id: &str) {
    record(
        evidence,
        &format!("define-{id}"),
        Event::ActionDescribed(ActionDescription {
            id: ActionDescriptionId::new(id).unwrap(),
            action_type: id.into(),
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
        test_sequence: TestSequenceId::new(run).unwrap(),
        observation_index: sequence,
        timestamp_micros: sequence * 1_000,
        recorded_state: RecordedState {
            id: RecordedStateId::new(format!("recorded_state-{id}")).unwrap(),
            properties: PropertyMap::from([(
                "sut:screen".into(),
                json!(state),
            )]),
            widgets: Vec::new(),
            capture: CaptureScope::Complete,
        },
        available_actions: actions
            .iter()
            .map(|action| AvailableAction {
                action_description: ActionDescriptionId::new(*action).unwrap(),
                target_widget: None,
            })
            .collect(),
        available_action_scope: AvailableActionScope::Generated,
        artifacts: Vec::new(),
    }
}

fn observe(evidence: &mut ReconstructedEvidence, observation: Observation) {
    let event_id = format!("observe-{}", observation.id);
    record(evidence, &event_id, Event::ObservationRecorded(observation));
}

fn execution(
    run: &str,
    id: &str,
    source: &str,
    action: &str,
) -> ActionExecution {
    ActionExecution {
        id: ActionExecutionId::new(id).unwrap(),
        test_sequence: TestSequenceId::new(run).unwrap(),
        source_observation: ObservationId::new(source).unwrap(),
        action_description: ActionDescriptionId::new(action).unwrap(),
        target_widget: None,
        parameters: PropertyMap::new(),
    }
}

fn select(
    evidence: &mut ReconstructedEvidence,
    run: &str,
    id: &str,
    source: &str,
    action: &str,
) {
    record(
        evidence,
        &format!("select-{id}"),
        Event::ActionSelected(execution(run, id, source, action)),
    );
}

fn submit(evidence: &mut ReconstructedEvidence, id: &str, accepted: bool) {
    record(
        evidence,
        &format!("submit-{id}"),
        Event::ActionExecutionReported(ActionExecutionResult {
            action_execution: ActionExecutionId::new(id).unwrap(),
            outcome: if accepted {
                ActionOutcome::Returned
            } else {
                ActionOutcome::Error
            },
            error: (!accepted).then(|| "submission rejected".into()),
        }),
    );
}

fn perform(
    evidence: &mut ReconstructedEvidence,
    id: &str,
    source: &str,
    action: &str,
    target: Observation,
) {
    select(evidence, target.test_sequence.as_str(), id, source, action);
    submit(evidence, id, true);
    let observation = target.id.clone();
    observe(evidence, target);
    record(
        evidence,
        &format!("result-{id}"),
        Event::TransitionRecorded {
            action_execution: ActionExecutionId::new(id).unwrap(),
            resulting_observation: observation,
        },
    );
}

#[test]
fn observations_are_distinct_and_queries_preserve_run_sequence() {
    let mut evidence = evidence();
    start_run(&mut evidence, "test_sequence");
    for (id, sequence, screen) in [("z", 1, "A"), ("a", 2, "B"), ("m", 3, "A")]
    {
        observe(
            &mut evidence,
            observation("test_sequence", id, sequence, screen, &[]),
        );
    }
    assert_eq!(evidence.recorded_state_count(), 3);
    assert_eq!(evidence.observation_count(), 3);
    assert_eq!(evidence.recorded_transition_count(), 0);
    assert_eq!(
        evidence
            .observations_for_sequence(
                &TestSequenceId::new("test_sequence").unwrap()
            )
            .iter()
            .map(|observation| observation.id.as_str())
            .collect::<Vec<_>>(),
        vec!["z", "a", "m"]
    );
    let recorded_state = RecordedStateId::new("recorded_state-z").unwrap();
    assert_eq!(
        evidence.observations_for_recorded_state(&recorded_state).len(),
        1
    );
    assert_eq!(
        evidence.recorded_state(&recorded_state).unwrap().properties["sut:screen"],
        json!("A")
    );
    assert_eq!(evidence.recorded_states().count(), 3);
    assert!(
        evidence
            .observations_for_sequence(&TestSequenceId::new("missing").unwrap())
            .is_empty()
    );
    assert!(
        evidence
            .observations_for_recorded_state(
                &RecordedStateId::new("missing").unwrap()
            )
            .is_empty()
    );
}

#[test]
fn identical_recorded_states_can_be_reused_without_merging_observations() {
    let mut evidence = evidence();
    start_run(&mut evidence, "test_sequence");
    let first = observation("test_sequence", "first", 1, "A", &[]);
    observe(&mut evidence, first.clone());
    let mut revisit = observation("test_sequence", "revisit", 2, "A", &[]);
    revisit.recorded_state = first.recorded_state.clone();
    observe(&mut evidence, revisit);
    assert_eq!(evidence.recorded_state_count(), 1);
    assert_eq!(evidence.observation_count(), 2);
    assert_eq!(
        evidence
            .observations_for_recorded_state(&first.recorded_state.id)
            .len(),
        2
    );
    let mut conflict = observation("test_sequence", "conflict", 3, "B", &[]);
    conflict.recorded_state.id = first.recorded_state.id.clone();
    let before = evidence.event_count();
    assert!(matches!(
        evidence.apply(envelope(
            "bad-recorded_state",
            Event::ObservationRecorded(conflict)
        )),
        Err(Error::Conflict(_))
    ));
    assert_eq!(evidence.event_count(), before);
    assert_eq!(
        evidence.recorded_state(&first.recorded_state.id),
        Some(&first.recorded_state)
    );
}

#[test]
fn repeated_visits_keep_separate_forward_links_and_execution_order() {
    let mut evidence = evidence();
    start_run(&mut evidence, "test_sequence");
    define_action(&mut evidence, "activate");
    observe(
        &mut evidence,
        observation("test_sequence", "first", 1, "A", &["activate"]),
    );
    perform(
        &mut evidence,
        "z-first",
        "first",
        "activate",
        observation("test_sequence", "second", 2, "A", &["activate"]),
    );
    perform(
        &mut evidence,
        "a-second",
        "second",
        "activate",
        observation("test_sequence", "third", 3, "A", &[]),
    );
    assert_eq!(evidence.recorded_state_count(), 3);
    assert_eq!(evidence.recorded_transition_count(), 2);
    assert_eq!(evidence.action_execution_count(), 2);
    let links = evidence.recorded_transitions().iter().collect::<Vec<_>>();
    assert_eq!(links[0].source_observation.as_str(), "first");
    assert_eq!(links[0].resulting_observation.as_str(), "second");
    assert_eq!(links[1].source_observation.as_str(), "second");
    assert_eq!(links[1].resulting_observation.as_str(), "third");
    assert_ne!(links[0].action_execution, links[1].action_execution);
    assert_eq!(
        evidence
            .executions_for_sequence(
                &TestSequenceId::new("test_sequence").unwrap()
            )
            .iter()
            .map(|record| record.action_execution.id.as_str())
            .collect::<Vec<_>>(),
        vec!["z-first", "a-second"]
    );
    assert!(
        evidence
            .incoming(&ObservationId::new("first").unwrap())
            .is_empty()
    );
    assert!(
        evidence
            .outgoing(&ObservationId::new("third").unwrap())
            .is_empty()
    );
    assert_eq!(
        evidence.outgoing(&ObservationId::new("first").unwrap()),
        vec![links[0]]
    );
    assert_eq!(
        evidence.incoming(&ObservationId::new("third").unwrap()),
        vec![links[1]]
    );
    assert!(
        !evidence
            .apply(envelope(
                "result-a-second",
                Event::TransitionRecorded {
                    action_execution: ActionExecutionId::new("a-second")
                        .unwrap(),
                    resulting_observation: ObservationId::new("third").unwrap()
                }
            ))
            .unwrap()
    );
    assert_eq!(evidence.recorded_transition_count(), 2);
}

#[test]
fn available_actions_parameters_and_attempts_remain_attached_to_their_occurrences()
 {
    let mut evidence = evidence();
    start_run(&mut evidence, "test_sequence");
    for action in ["activate", "confirm", "restart"] {
        define_action(&mut evidence, action);
    }
    observe(
        &mut evidence,
        observation("test_sequence", "first", 1, "A", &["activate", "confirm"]),
    );
    let mut selected =
        execution("test_sequence", "attempt", "first", "confirm");
    selected
        .parameters
        .insert("input.text".into(), json!("retained input"));
    record(
        &mut evidence,
        "selection",
        Event::ActionSelected(selected.clone()),
    );
    submit(&mut evidence, "attempt", false);
    observe(
        &mut evidence,
        observation(
            "test_sequence",
            "second",
            2,
            "A",
            &["activate", "restart"],
        ),
    );
    assert_eq!(
        evidence
            .available_actions(&ObservationId::new("second").unwrap())
            .unwrap()
            .iter()
            .map(|candidate| candidate.action_description.as_str())
            .collect::<Vec<_>>(),
        vec!["activate", "restart"]
    );
    assert_eq!(
        evidence
            .action_execution(&selected.id)
            .unwrap()
            .action_execution,
        selected
    );
    assert_eq!(evidence.action_description_count(), 3);
    assert_eq!(evidence.action_descriptions().count(), 3);
    assert!(
        evidence
            .action_description(&ActionDescriptionId::new("confirm").unwrap())
            .is_some()
    );
    assert!(
        evidence
            .action_description(&ActionDescriptionId::new("absent").unwrap())
            .is_none()
    );
    assert_eq!(evidence.test_sequences().count(), 1);
    assert_eq!(evidence.recorded_transition_count(), 0);
}

#[test]
fn results_cannot_point_to_the_source_or_an_earlier_observation() {
    let mut evidence = evidence();
    start_run(&mut evidence, "test_sequence");
    define_action(&mut evidence, "activate");
    observe(
        &mut evidence,
        observation("test_sequence", "earlier", 0, "A", &[]),
    );
    observe(
        &mut evidence,
        observation(
            "test_sequence",
            "source_observation",
            1,
            "A",
            &["activate"],
        ),
    );
    select(
        &mut evidence,
        "test_sequence",
        "attempt",
        "source_observation",
        "activate",
    );
    submit(&mut evidence, "attempt", true);
    let before = evidence.event_count();
    for target in ["earlier", "source_observation"] {
        assert!(matches!(
            evidence.apply(envelope(
                target,
                Event::TransitionRecorded {
                    action_execution: ActionExecutionId::new("attempt")
                        .unwrap(),
                    resulting_observation: ObservationId::new(target).unwrap()
                }
            )),
            Err(Error::Validation {
                code: "invalid_transition",
                ..
            })
        ));
    }
    assert_eq!(evidence.event_count(), before);
    assert_eq!(evidence.recorded_transition_count(), 0);
}

#[test]
fn exact_event_replay_is_idempotent_and_conflicts_do_not_mutate_the_evidence() {
    let mut evidence = evidence();
    start_run(&mut evidence, "run-1");
    let observed = observation("run-1", "first", 0, "main", &[]);
    let original =
        envelope("first-event", Event::ObservationRecorded(observed.clone()));
    assert!(evidence.apply(original.clone()).unwrap());
    let event_count = evidence.event_count();

    assert!(!evidence.apply(original).unwrap());
    assert_eq!(evidence.event_count(), event_count);
    assert_eq!(evidence.observation_count(), 1);

    let mut changed = observed.clone();
    changed.timestamp_micros += 1;
    let collision =
        envelope("first-event", Event::ObservationRecorded(changed));
    assert!(matches!(evidence.apply(collision), Err(Error::Conflict(_))));

    let invalid = envelope(
        "invalid-event",
        Event::ObservationRecorded(observation(
            "absent",
            "missing-run",
            1,
            "other",
            &[],
        )),
    );
    assert!(matches!(
        evidence.apply(invalid),
        Err(Error::Validation {
            code: "invalid_reference",
            ..
        })
    ));
    assert_eq!(evidence.event_count(), event_count);
    assert_eq!(evidence.observation(&observed.id), Some(&observed));
    assert_eq!(evidence.recorded_state_count(), 1);
    assert_eq!(evidence.recorded_transition_count(), 0);
}

#[test]
fn initial_observations_and_cross_sequence_references_never_invent_edges() {
    let mut evidence = evidence();
    start_run(&mut evidence, "run-1");
    start_run(&mut evidence, "run-2");
    define_action(&mut evidence, "activate");
    observe(
        &mut evidence,
        observation("run-1", "first", 0, "source_observation", &["activate"]),
    );
    observe(
        &mut evidence,
        observation("run-2", "other", 0, "target_widget", &["activate"]),
    );
    select(&mut evidence, "run-1", "attempt-1", "first", "activate");
    submit(&mut evidence, "attempt-1", true);
    let event_count = evidence.event_count();

    let invalid_result = envelope(
        "cross-result",
        Event::TransitionRecorded {
            action_execution: ActionExecutionId::new("attempt-1").unwrap(),
            resulting_observation: ObservationId::new("other").unwrap(),
        },
    );
    assert!(matches!(
        evidence.apply(invalid_result),
        Err(Error::Validation {
            code: "invalid_transition",
            ..
        })
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
        evidence.apply(invalid_selection),
        Err(Error::Validation {
            code: "invalid_reference",
            ..
        })
    ));

    assert_eq!(evidence.event_count(), event_count);
    assert_eq!(evidence.recorded_state_count(), 2);
    assert_eq!(evidence.recorded_transition_count(), 0);
    let pending = evidence
        .action_execution(&ActionExecutionId::new("attempt-1").unwrap())
        .unwrap();
    assert_eq!(
        pending.status,
        ActionExecutionStatus::AwaitingResultingObservation
    );
    assert!(pending.resulting_observation.is_none());
}

#[test]
fn rejected_and_unfinished_actions_preserve_status_without_fabricating_results()
{
    let mut evidence = evidence();
    start_run(&mut evidence, "run-1");
    define_action(&mut evidence, "activate");
    observe(
        &mut evidence,
        observation("run-1", "first", 0, "main", &["activate"]),
    );
    select(&mut evidence, "run-1", "rejected", "first", "activate");
    submit(&mut evidence, "rejected", false);
    select(&mut evidence, "run-1", "not-submitted", "first", "activate");
    observe(
        &mut evidence,
        observation("run-1", "second", 1, "other", &["activate"]),
    );
    let invalid_result = envelope(
        "missing-submission-result",
        Event::TransitionRecorded {
            action_execution: ActionExecutionId::new("not-submitted").unwrap(),
            resulting_observation: ObservationId::new("second").unwrap(),
        },
    );
    assert!(matches!(
        evidence.apply(invalid_result),
        Err(Error::Validation {
            code: "invalid_lifecycle",
            ..
        })
    ));
    record(
        &mut evidence,
        "finish-run-1",
        Event::TestSequenceFinished {
            test_sequence: TestSequenceId::new("run-1").unwrap(),
            reason: "interrupted".into(),
        },
    );

    start_run(&mut evidence, "run-2");
    observe(
        &mut evidence,
        observation("run-2", "third", 0, "main", &["activate"]),
    );
    select(&mut evidence, "run-2", "submitted", "third", "activate");
    submit(&mut evidence, "submitted", true);
    record(
        &mut evidence,
        "finish-run-2",
        Event::TestSequenceFinished {
            test_sequence: TestSequenceId::new("run-2").unwrap(),
            reason: "driver disconnected".into(),
        },
    );

    assert_eq!(
        evidence
            .action_execution(&ActionExecutionId::new("rejected").unwrap())
            .unwrap()
            .status,
        ActionExecutionStatus::Failed
    );
    for id in ["not-submitted", "submitted"] {
        let record = evidence
            .action_execution(&ActionExecutionId::new(id).unwrap())
            .unwrap();
        assert_eq!(record.status, ActionExecutionStatus::Incomplete);
        assert!(record.resulting_observation.is_none());
    }
    assert!(matches!(
        evidence.test_sequence_status(&TestSequenceId::new("run-1").unwrap()),
        Some(HistoryStatus::Finished { .. })
    ));
    assert_eq!(evidence.recorded_transition_count(), 0);
}

#[test]
fn checks_preserve_scope_and_pending_results_without_inferred_passes() {
    let mut evidence = evidence();
    start_run(&mut evidence, "run-1");
    start_run(&mut evidence, "run-2");
    observe(&mut evidence, observation("run-1", "first", 0, "main", &[]));
    observe(&mut evidence, observation("run-2", "other", 0, "main", &[]));
    let pending = OracleResult {
        test_run: TestRunId::new(format!("parent-{}", "run-1")).unwrap(),
        test_sequence: Some(TestSequenceId::new("run-1").unwrap()),
        oracle_severity: None,
        oracle_target: OracleTarget::TestSequence {},
        oracle: Oracle {
            id: "example:eventually-completes".into(),
            version: None,
            parameters: None,
        },
        expected_value: ExpectedValue::Unspecified,
        observed_value: None,
        name: "operation eventually completes".into(),
        oracle_status: OracleStatus::Pending,
        oracle_details: PropertyMap::new(),
    };
    record(
        &mut evidence,
        "pending-check",
        Event::OracleResultRecorded(pending.clone()),
    );
    let failed = OracleResult {
        oracle_target: OracleTarget::Observation {
            observation: ObservationId::new("first").unwrap(),
        },
        name: "button is enabled".into(),
        oracle_status: OracleStatus::Failed,
        ..pending.clone()
    };
    record(
        &mut evidence,
        "failed-check",
        Event::OracleResultRecorded(failed.clone()),
    );

    let invalid = OracleResult {
        oracle_target: OracleTarget::Observation {
            observation: ObservationId::new("other").unwrap(),
        },
        ..pending.clone()
    };
    assert!(matches!(
        evidence.apply(envelope(
            "cross-check",
            Event::OracleResultRecorded(invalid)
        )),
        Err(Error::Validation {
            code: "invalid_oracle_target",
            field: "oracle_target.observation",
            ..
        })
    ));
    assert_eq!(evidence.oracle_results(), &[pending, failed]);
    assert!(
        !evidence
            .oracle_results()
            .iter()
            .any(|check| check.oracle_status == OracleStatus::Passed)
    );
    assert_eq!(evidence.recorded_state_count(), 2);
}

#[test]
fn ordering_stale_sources_and_unknown_available_actions_are_rejected_without_partial_updates()
 {
    let mut evidence = evidence();
    start_run(&mut evidence, "run-1");
    define_action(&mut evidence, "activate");
    observe(
        &mut evidence,
        observation("run-1", "first", 1, "main", &["activate"]),
    );
    observe(
        &mut evidence,
        observation("run-1", "second", 2, "main", &["activate"]),
    );
    let event_count = evidence.event_count();

    for (id, sequence) in [("same-sequence", 2), ("earlier-sequence", 0)] {
        let invalid =
            observation("run-1", id, sequence, "unexpected", &["activate"]);
        assert!(matches!(
            evidence.apply(envelope(id, Event::ObservationRecorded(invalid))),
            Err(Error::Validation {
                code: "invalid_order",
                ..
            })
        ));
    }
    let stale =
        Event::ActionSelected(execution("run-1", "stale", "first", "activate"));
    assert!(matches!(
        evidence.apply(envelope("stale-source", stale)),
        Err(Error::Validation {
            code: "invalid_reference",
            ..
        })
    ));
    let missing_candidate = Event::ActionSelected(execution(
        "run-1", "unknown", "second", "absent",
    ));
    assert!(matches!(
        evidence.apply(envelope("unknown-candidate", missing_candidate)),
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
        evidence.apply(envelope(
            "unknown-definition",
            Event::ObservationRecorded(missing_definition)
        )),
        Err(Error::UnknownReference(_))
    ));

    assert_eq!(evidence.event_count(), event_count);
    assert_eq!(evidence.observation_count(), 2);
    assert_eq!(evidence.recorded_state_count(), 2);
    assert_eq!(evidence.recorded_transition_count(), 0);
    assert!(
        evidence
            .action_execution(&ActionExecutionId::new("stale").unwrap())
            .is_none()
    );
}
