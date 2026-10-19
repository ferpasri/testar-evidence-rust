use std::path::PathBuf;

use serde_json::json;
use testar_state_model::*;

fn record(
    store: &mut JsonlStore,
    sequence: &mut u64,
    event: Event,
) -> Result<()> {
    // Give each recorded event a unique ID within this example.
    *sequence += 1;
    store.record(EventEnvelope {
        schema_version: SCHEMA_VERSION,
        id: EventId::new(format!("example:event:{sequence}"))?,
        event,
    })?;
    Ok(())
}

fn main() -> Result<()> {
    // The first command-line argument selects where to save the model.
    let directory =
        std::env::args_os()
            .nth(1)
            .map(PathBuf::from)
            .ok_or_else(|| {
                Error::InvalidRecord(
                    "provide a new model directory as the first argument"
                        .into(),
                )
            })?;
    // Create the model directory and identify its data format.
    let manifest = Manifest {
        schema_version: SCHEMA_VERSION,
        model: ModelId::new("example")?,
    };
    let mut store = JsonlStore::create(&directory, manifest.clone())?;
    // Store a sample screenshot so an observation can reference it.
    let image = store.put_artifact(
        include_bytes!("../tests/fixtures/screenshot.png"),
        "image/png",
        Some(1),
        Some(1),
    )?;
    let run = RunId::new("example:run:1")?;
    let action = ActionId::new("example:activate")?;
    let widget = WidgetId::new("login")?;
    let source = ObservationId::new("example:observation:1")?;
    let target = ObservationId::new("example:observation:2")?;
    let execution = ExecutionId::new("example:execution:1")?;
    let mut sequence = 0;
    // Start the run and define the action that will be recorded.
    record(
        &mut store,
        &mut sequence,
        Event::RunStarted(Run {
            id: run.clone(),
            producer: "standalone-example".into(),
            source_run: "1".into(),
            properties: PropertyMap::from([(
                "sut.name".into(),
                json!("example"),
            )]),
        }),
    )?;
    record(
        &mut store,
        &mut sequence,
        Event::ActionDefined(ActionDefinition {
            id: action.clone(),
            kind: "activate".into(),
            properties: PropertyMap::new(),
        }),
    )?;
    let state = ConcreteState {
        id: ConcreteStateId::new("example:snapshot:1")?,
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
        Event::ActionSubmitted {
            execution: execution.clone(),
            accepted: true,
            error: None,
        },
    )?;
    let state = ConcreteState {
        id: ConcreteStateId::new("example:snapshot:2")?,
        properties: PropertyMap::from([("ui.title".into(), json!("Welcome"))]),
        widgets: vec![],
        capture: CaptureScope::Filtered,
    };
    record(
        &mut store,
        &mut sequence,
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
        Event::ActionResult {
            execution,
            observation: target,
        },
    )?;
    record(
        &mut store,
        &mut sequence,
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
        Event::Checked(CheckResult {
            run: RunId::new("example:run:1")?,
            target: CheckTarget::Observation {
                observation: ObservationId::new("example:observation:2")?,
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
