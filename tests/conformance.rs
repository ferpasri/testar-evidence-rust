mod support;

use std::fs;
use std::path::Path;

use serde_json::{Value, json};
use tempfile::tempdir;
use testar_evidence::{
    Error, Event, EventEnvelope, EventId, JsonlStore, Manifest, RunStatus,
    StateModel,
};

fn read_json(path: &Path) -> Value {
    serde_json::from_slice(&fs::read(path).unwrap()).unwrap()
}

fn inputs(source: &Path) -> (Manifest, Vec<EventEnvelope>) {
    let manifest =
        serde_json::from_value(read_json(&source.join("manifest.json")))
            .unwrap();
    let events = fs::read_to_string(source.join("evidence.jsonl"))
        .unwrap()
        .lines()
        .map(|line| {
            let raw: Value = serde_json::from_str(line).unwrap();
            let envelope: EventEnvelope =
                serde_json::from_value(raw.clone()).unwrap();
            assert_eq!(
                serde_json::to_value(&envelope).unwrap(),
                raw,
                "Fields must survive decoding in {}",
                source.display()
            );
            envelope
        })
        .collect();
    (manifest, events)
}

fn copy_directory(source: &Path, target: &Path) {
    fs::create_dir_all(target).unwrap();
    for entry in fs::read_dir(source).unwrap() {
        let entry = entry.unwrap();
        let destination = target.join(entry.file_name());
        if entry.file_type().unwrap().is_dir() {
            copy_directory(&entry.path(), &destination);
        } else {
            fs::copy(entry.path(), destination).unwrap();
        }
    }
}

fn normalized(model: &StateModel) -> Value {
    let observations: Vec<_> = model.observations().map(|observation| json!({
        "id": observation.id,
        "run": observation.run,
        "sequence": observation.sequence,
        "concrete_state": observation.state.id,
        "capture": observation.state.capture,
        "candidate_scope": observation.candidate_scope,
        "widget_ids": observation.state.widgets.iter().map(|widget| &widget.id).collect::<Vec<_>>(),
        "candidates": observation.candidates,
        "artifact_ids": observation.artifacts.iter().map(|artifact| &artifact.id).collect::<Vec<_>>(),
    })).collect();
    let runs: Vec<_> = model
        .runs()
        .map(|run| {
            let status = match model.run_status(&run.id).unwrap() {
                RunStatus::Running => "running",
                RunStatus::Finished { .. } => "finished",
            };
            json!({"id": run.id, "status": status})
        })
        .collect();
    json!({
        "status": "accepted",
        "counts": {
            "events": model.event_count(),
            "runs": model.run_count(),
            "observations": model.observation_count(),
            "actions": model.action_count(),
            "executions": model.execution_count(),
            "checks": model.checks().len(),
            "artifact_references": model.observations().map(|observation| observation.artifacts.len()).sum::<usize>(),
            "concrete_states": model.concrete_state_count(),
        },
        "runs": runs,
        "observations": observations,
        "actions": model.actions().map(|action| json!({"id": action.id, "kind": action.kind})).collect::<Vec<_>>(),
        "executions": model.executions().map(|record| json!({
            "execution": record.execution, "status": record.status, "result": record.result, "error": record.error,
        })).collect::<Vec<_>>(),
        "checks": model.checks(),
        "concrete_states": model.concrete_states().map(|state| &state.id).collect::<Vec<_>>(),
    })
}

fn normalized_error(error: &Error, event: &EventId) -> Value {
    match error {
        Error::InvalidCheckTarget { field, .. } => json!({
            "code": "invalid_check_target", "event": event, "field": field,
        }),
        _ => panic!("Unexpected error for event {event}: {error}"),
    }
}

fn assert_result(model: &StateModel, expected: &Value, failure: Option<Value>) {
    match expected["status"].as_str().unwrap() {
        "accepted" => {
            assert_eq!(failure, None);
            assert_eq!(normalized(model), *expected);
        }
        "rejected" => {
            assert_eq!(failure, Some(expected["error"].clone()));
            assert_eq!(normalized(model), expected["accepted_prefix"]);
        }
        status => panic!("Unsupported expected status: {status}"),
    }
}

fn assert_evidence_preserved(
    store: &JsonlStore,
    directory: &Path,
    source: &Path,
    accepted: &[EventEnvelope],
) {
    let persisted: Vec<Value> =
        fs::read_to_string(directory.join("evidence.jsonl"))
            .unwrap()
            .lines()
            .map(|line| serde_json::from_str(line).unwrap())
            .collect();
    let expected: Vec<Value> = accepted
        .iter()
        .map(|envelope| serde_json::to_value(envelope).unwrap())
        .collect();
    assert_eq!(
        persisted,
        expected,
        "Accepted event payloads must survive persistence in {}",
        source.display()
    );
    for envelope in accepted {
        match &envelope.event {
            Event::Observed(observation) => {
                assert_eq!(
                    store.model().observation(&observation.id),
                    Some(observation)
                );
                for artifact in &observation.artifacts {
                    assert_eq!(
                        store.read_artifact(artifact).unwrap(),
                        fs::read(source.join(&artifact.path)).unwrap()
                    );
                }
            }
            Event::RunStarted(run) => {
                assert_eq!(store.model().run(&run.id), Some(run))
            }
            Event::ActionDefined(action) => {
                assert_eq!(store.model().action(&action.id), Some(action))
            }
            Event::ActionSelected(execution) => assert_eq!(
                &store.model().execution(&execution.id).unwrap().execution,
                execution
            ),
            Event::Checked(check) => {
                assert!(store.model().checks().contains(check))
            }
            Event::RunFinished { run, reason } => assert_eq!(
                store.model().run_status(run),
                Some(&RunStatus::Finished {
                    reason: reason.clone()
                })
            ),
            _ => {}
        }
    }
}

#[test]
fn prepared_release_contains_discoverable_cases() {
    assert!(
        !xtask::case_directories(support::specs_root())
            .unwrap()
            .is_empty()
    );
}

#[test]
fn reads_every_shared_case_and_preserves_evidence_on_rejection() {
    for source in xtask::case_directories(support::specs_root()).unwrap() {
        eprintln!("Reader case: {}", source.display());
        let expected = read_json(&source.join("expected.json"));
        let (manifest, events) = inputs(&source);
        let directory = tempdir().unwrap();
        copy_directory(&source, directory.path());
        let manifest_before =
            fs::read(directory.path().join("manifest.json")).unwrap();
        let evidence_before =
            fs::read(directory.path().join("evidence.jsonl")).unwrap();
        match JsonlStore::open(directory.path()) {
            Ok(store) => {
                assert_result(store.model(), &expected, None);
                assert_evidence_preserved(
                    &store,
                    directory.path(),
                    &source,
                    &events,
                );
            }
            Err(reader_error) => {
                let mut model = StateModel::new(manifest).unwrap();
                let mut accepted = Vec::new();
                let mut failure = None;
                for envelope in events {
                    let before = normalized(&model);
                    match model.apply(envelope.clone()) {
                        Ok(_) => accepted.push(envelope),
                        Err(error) => {
                            assert_eq!(
                                normalized(&model),
                                before,
                                "Rejected event changed the model"
                            );
                            let actual = normalized_error(&error, &envelope.id);
                            assert_eq!(
                                normalized_error(&reader_error, &envelope.id),
                                actual
                            );
                            failure = Some(actual);
                            break;
                        }
                    }
                }
                assert_result(&model, &expected, failure);
                let prefix = tempdir().unwrap();
                copy_directory(&source, prefix.path());
                let mut evidence = Vec::new();
                for envelope in &accepted {
                    serde_json::to_writer(&mut evidence, envelope).unwrap();
                    evidence.push(b'\n');
                }
                fs::write(prefix.path().join("evidence.jsonl"), evidence)
                    .unwrap();
                let reopened = JsonlStore::open(prefix.path()).unwrap();
                assert_eq!(normalized(reopened.model()), normalized(&model));
                assert_evidence_preserved(
                    &reopened,
                    prefix.path(),
                    &source,
                    &accepted,
                );
            }
        }
        assert_eq!(
            fs::read(directory.path().join("manifest.json")).unwrap(),
            manifest_before
        );
        assert_eq!(
            fs::read(directory.path().join("evidence.jsonl")).unwrap(),
            evidence_before
        );
    }
}

#[test]
fn produces_every_shared_case_and_reopens_its_accepted_prefix() {
    for source in xtask::case_directories(support::specs_root()).unwrap() {
        eprintln!("Producer case: {}", source.display());
        let expected = read_json(&source.join("expected.json"));
        let (manifest, events) = inputs(&source);
        let directory = tempdir().unwrap();
        let mut store = JsonlStore::create(directory.path(), manifest).unwrap();
        let mut accepted = Vec::new();
        let mut failure = None;
        for envelope in events {
            if let Event::Observed(observation) = &envelope.event {
                for artifact in &observation.artifacts {
                    let generated = store
                        .put_artifact(
                            &fs::read(source.join(&artifact.path)).unwrap(),
                            &artifact.media_type,
                            artifact.width,
                            artifact.height,
                        )
                        .unwrap();
                    assert_eq!(&generated, artifact);
                }
            }
            let before = normalized(store.model());
            let log_before =
                fs::read(directory.path().join("evidence.jsonl")).unwrap();
            match store.record(envelope.clone()) {
                Ok(applied) => {
                    assert!(applied);
                    assert!(
                        !store.record(envelope.clone()).unwrap(),
                        "Exact retry duplicated an event"
                    );
                    accepted.push(envelope);
                }
                Err(error) => {
                    assert_eq!(normalized(store.model()), before);
                    assert_eq!(
                        fs::read(directory.path().join("evidence.jsonl"))
                            .unwrap(),
                        log_before
                    );
                    failure = Some(normalized_error(&error, &envelope.id));
                    break;
                }
            }
        }
        assert_result(store.model(), &expected, failure.clone());
        assert_evidence_preserved(&store, directory.path(), &source, &accepted);
        drop(store);
        let reopened = JsonlStore::open(directory.path()).unwrap();
        assert_result(reopened.model(), &expected, failure);
        assert_evidence_preserved(
            &reopened,
            directory.path(),
            &source,
            &accepted,
        );
        assert_eq!(
            read_json(&directory.path().join("manifest.json")),
            read_json(&source.join("manifest.json"))
        );
    }
}
