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
    // Create the model directory and identify its data format.
    let manifest = Manifest {
        schema_version: SCHEMA_VERSION,
        model: ModelId::new(format!("example:model:{identity}"))?,
    };
    let mut store = JsonlStore::create(&directory, manifest.clone())?;
    // Store a sample screenshot so an observation can reference it.
    let image = store.put_artifact(
        include_bytes!("../tests/fixtures/screenshot.png"),
        "image/png",
        Some(1),
        Some(1),
    )?;
    let run = RunId::new(format!("example:{identity}:run:1"))?;
    let action = ActionId::new(format!("example:{identity}:activate"))?;
    let widget = WidgetId::new("login")?;
    let source =
        ObservationId::new(format!("example:{identity}:observation:1"))?;
    let target =
        ObservationId::new(format!("example:{identity}:observation:2"))?;
    let execution =
        ExecutionId::new(format!("example:{identity}:execution:1"))?;
    let mut sequence = 0;
    // Start the run and define the action that will be recorded.
    record(
        &mut store,
        &mut sequence,
        &identity,
        Event::RunStarted(Run {
            id: run.clone(),
            producer: "standalone-example".into(),
            source_run: identity.clone(),
            properties,
        }),
    )?;
    record(
        &mut store,
        &mut sequence,
        &identity,
        Event::ActionDefined(ActionDefinition {
            id: action.clone(),
            kind: "activate".into(),
            properties: PropertyMap::new(),
        }),
    )?;
    let state = ConcreteState {
        id: ConcreteStateId::new(format!("example:{identity}:snapshot:1"))?,
        properties: PropertyMap::from([("ui.title".into(), json!("Login"))]),
        widgets: vec![Widget {
            id: widget.clone(),
            parent: None,
            properties: PropertyMap::from([
                ("ui.role".into(), json!("button")),
                ("ui.text".into(), json!("Login")),
            ]),
        }],
        capture: CaptureScope::Filtered,
    };
    record(
        &mut store,
        &mut sequence,
        &identity,
        Event::Observed(Observation {
            id: source.clone(),
            run: run.clone(),
            sequence: 1,
            timestamp_micros: 1000,
            state,
            candidate_scope: CandidateScope::Generated,
            candidates: vec![ActionCandidate {
                action: action.clone(),
                target: Some(widget.clone()),
            }],
            artifacts: vec![image],
        }),
    )?;
    record(
        &mut store,
        &mut sequence,
        &identity,
        Event::ActionSelected(Execution {
            id: execution.clone(),
            run: run.clone(),
            source,
            action,
            target: Some(widget),
            parameters: PropertyMap::new(),
        }),
    )?;
    record(
        &mut store,
        &mut sequence,
        &identity,
        Event::ActionSubmitted {
            execution: execution.clone(),
            accepted: true,
            error: None,
        },
    )?;
    let state = ConcreteState {
        id: ConcreteStateId::new(format!("example:{identity}:snapshot:2"))?,
        properties: PropertyMap::from([("ui.title".into(), json!("Welcome"))]),
        widgets: vec![],
        capture: CaptureScope::Filtered,
    };
    record(
        &mut store,
        &mut sequence,
        &identity,
        Event::Observed(Observation {
            id: target.clone(),
            run: run.clone(),
            sequence: 2,
            timestamp_micros: 2000,
            state,
            candidate_scope: CandidateScope::Generated,
            candidates: vec![],
            artifacts: vec![],
        }),
    )?;
    record(
        &mut store,
        &mut sequence,
        &identity,
        Event::ActionResult {
            execution,
            observation: target,
        },
    )?;
    record(
        &mut store,
        &mut sequence,
        &identity,
        Event::RunFinished {
            run,
            reason: "example complete".into(),
        },
    )?;
    // Release the writer, then reopen the saved evidence.
    drop(store);
    let mut reopened = JsonlStore::open(&directory)?;
    // A check result can arrive after the run finishes. This review is indeterminate.
    record(
        &mut reopened,
        &mut sequence,
        &identity,
        Event::Checked(CheckResult {
            run: RunId::new(format!("example:{identity}:run:1"))?,
            target: CheckTarget::Observation {
                observation: ObservationId::new(format!(
                    "example:{identity}:observation:2"
                ))?,
            },
            rule: CheckRule {
                id: "example:manual-review".into(),
                version: None,
                parameters: None,
            },
            expected: ExpectedValue::Unspecified,
            actual: None,
            name: "manual-review".into(),
            status: CheckStatus::Unknown,
            details: PropertyMap::new(),
        }),
    )?;
    println!(
        "Reopened {} concrete snapshots, {} execution links, {} observations",
        reopened.model().concrete_state_count(),
        reopened.model().link_count(),
        reopened.model().observation_count()
    );
    Ok(())
}
