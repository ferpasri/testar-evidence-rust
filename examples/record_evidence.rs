mod support;

use std::path::PathBuf;

use serde_json::json;
use testar_evidence::*;

fn record(
    store: &mut JsonlStore,
    sequence: &mut u64,
    identity: &str,
    event: Event,
) -> Result<()> {
    // Give each recorded event a unique ID within this example.
    *sequence += 1;
    store.record(EvidenceEvent {
        schema_version: SCHEMA_VERSION,
        id: EventId::new(format!("example:{identity}:event:{sequence}"))?,
        event,
    })?;
    Ok(())
}

fn main() -> Result<()> {
    let arguments: Vec<_> = std::env::args_os().collect();
    let parent = arguments
        .get(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("target/traces"));
    let sut = arguments
        .get(2)
        .map(|value| value.to_string_lossy().into_owned())
        .unwrap_or_else(|| "demo".to_owned());
    let test_objective = arguments
        .get(3)
        .map(|value| value.to_string_lossy().into_owned());
    let origin = arguments
        .get(4)
        .map(|value| value.to_string_lossy().into_owned());
    let mut properties =
        PropertyMap::from([("example.sut".into(), json!(sut))]);
    for (key, value) in [
        ("evidence.origin", origin.as_deref()),
        ("evidence.test_objective", test_objective.as_deref()),
    ] {
        if let Some(value) = value {
            if value.trim().is_empty() {
                return Err(Error::InvalidRecord(
                    "Run descriptions must be nonblank".into(),
                ));
            }
            properties.insert(key.into(), json!(value));
        }
    }
    let directory = support::create_trace_directory(
        &parent,
        &sut,
        test_objective.as_deref().unwrap_or("run"),
    )?;
    let identity = uuid::Uuid::new_v4().to_string();
    println!("Evidence: {}", directory.display());
    // A collection can contain several runs; this example records one sequence in one run.
    let mut store = JsonlStore::create(
        &directory,
        Manifest {
            schema_version: SCHEMA_VERSION,
            evidence_id: EvidenceId::new(format!(
                "example:evidence:{identity}"
            ))?,
        },
    )?;
    let image = store.put_artifact(
        include_bytes!("../tests/fixtures/screenshot.png"),
        "image/png",
        Some(1),
        Some(1),
    )?;
    let run = TestRunId::new(format!("example:{identity}:run"))?;
    let test_sequence =
        TestSequenceId::new(format!("example:{identity}:sequence"))?;
    let action =
        ActionDescriptionId::new(format!("example:{identity}:activate"))?;
    let source =
        ObservationId::new(format!("example:{identity}:observation:1"))?;
    let resulting =
        ObservationId::new(format!("example:{identity}:observation:2"))?;
    let execution =
        ActionExecutionId::new(format!("example:{identity}:execution"))?;
    let widget = WidgetId::new("login")?;
    let mut event_index = 0;
    record(
        &mut store,
        &mut event_index,
        &identity,
        Event::TestRunStarted(TestRun {
            id: run.clone(),
            producer: "standalone-example".into(),
            source_run: identity.clone(),
            properties,
        }),
    )?;
    record(
        &mut store,
        &mut event_index,
        &identity,
        Event::TestSequenceStarted(TestSequence {
            id: test_sequence.clone(),
            test_run: run.clone(),
            sequence_index: 1,
            source_sequence: "1".into(),
            properties: PropertyMap::new(),
        }),
    )?;
    record(
        &mut store,
        &mut event_index,
        &identity,
        Event::ActionDescribed(ActionDescription {
            id: action.clone(),
            action_type: "activate".into(),
            properties: PropertyMap::new(),
        }),
    )?;
    // Save the available choice with its recorded state and a sample screenshot.
    record(
        &mut store,
        &mut event_index,
        &identity,
        Event::ObservationRecorded(Observation {
            id: source.clone(),
            test_sequence: test_sequence.clone(),
            observation_index: 1,
            timestamp_micros: 1000,
            recorded_state: RecordedState {
                id: RecordedStateId::new(format!("{identity}:state:1"))?,
                properties: PropertyMap::from([(
                    "ui.title".into(),
                    json!("Login"),
                )]),
                widgets: vec![Widget {
                    id: widget.clone(),
                    parent: None,
                    properties: PropertyMap::from([(
                        "ui.text".into(),
                        json!("Login"),
                    )]),
                }],
                capture: CaptureScope::Filtered,
            },
            available_action_scope: AvailableActionScope::Eligible,
            available_actions: vec![AvailableAction {
                action_description: action.clone(),
                target_widget: Some(widget.clone()),
            }],
            artifacts: vec![image],
        }),
    )?;
    record(
        &mut store,
        &mut event_index,
        &identity,
        Event::ActionSelected(ActionExecution {
            id: execution.clone(),
            test_sequence: test_sequence.clone(),
            source_observation: source,
            action_description: action,
            target_widget: Some(widget),
            parameters: PropertyMap::new(),
        }),
    )?;
    // A normal return and the resulting observation are separate evidence.
    record(
        &mut store,
        &mut event_index,
        &identity,
        Event::ActionExecutionReported(ActionExecutionResult {
            action_execution: execution.clone(),
            outcome: ActionOutcome::Returned,
            error: None,
        }),
    )?;
    record(
        &mut store,
        &mut event_index,
        &identity,
        Event::ObservationRecorded(Observation {
            id: resulting.clone(),
            test_sequence: test_sequence.clone(),
            observation_index: 2,
            timestamp_micros: 2000,
            recorded_state: RecordedState {
                id: RecordedStateId::new(format!("{identity}:state:2"))?,
                properties: PropertyMap::from([(
                    "ui.title".into(),
                    json!("Welcome"),
                )]),
                widgets: vec![],
                capture: CaptureScope::Filtered,
            },
            available_action_scope: AvailableActionScope::Unknown,
            available_actions: vec![],
            artifacts: vec![],
        }),
    )?;
    record(
        &mut store,
        &mut event_index,
        &identity,
        Event::TransitionRecorded {
            action_execution: execution,
            resulting_observation: resulting.clone(),
        },
    )?;
    record(
        &mut store,
        &mut event_index,
        &identity,
        Event::TestSequenceFinished {
            test_sequence: test_sequence.clone(),
            reason: "example complete".into(),
        },
    )?;
    record(
        &mut store,
        &mut event_index,
        &identity,
        Event::TestRunFinished {
            test_run: run.clone(),
            reason: "example complete".into(),
        },
    )?;
    // Reopening validates the saved trace. Oracle results may arrive after its histories finish.
    drop(store);
    let mut reopened = JsonlStore::open(&directory)?;
    record(
        &mut reopened,
        &mut event_index,
        &identity,
        Event::OracleResultRecorded(OracleResult {
            test_run: run,
            test_sequence: Some(test_sequence),
            name: "Manual review".into(),
            oracle: Oracle {
                id: "example:manual-review".into(),
                version: None,
                parameters: None,
            },
            oracle_target: OracleTarget::Observation {
                observation: resulting,
            },
            oracle_status: OracleStatus::Unknown,
            expected_value: ExpectedValue::Unspecified,
            observed_value: None,
            oracle_severity: None,
            oracle_details: PropertyMap::new(),
        }),
    )?;
    println!(
        "Reopened {} recorded states, {} recorded transitions, {} observations",
        reopened.evidence().recorded_state_count(),
        reopened.evidence().recorded_transition_count(),
        reopened.evidence().observation_count()
    );
    Ok(())
}
