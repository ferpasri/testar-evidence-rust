mod support;

use std::fs;
use std::path::Path;

use serde_json::{Value, json};
use tempfile::tempdir;
use testar_evidence::{
    Error, Event, EventId, EvidenceEvent, ReconstructedEvidence, HistoryStatus,
    JsonlStore, Manifest,
};

fn read_json(path: &Path) -> Value {
    serde_json::from_slice(&fs::read(path).unwrap()).unwrap()
}

fn inputs(source: &Path) -> (Manifest, Vec<EvidenceEvent>) {
    let manifest =
        serde_json::from_value(read_json(&source.join("manifest.json")))
            .unwrap();
    let events = fs::read_to_string(source.join("evidence.jsonl"))
        .unwrap()
        .lines()
        .map(|line| {
            let raw: Value = serde_json::from_str(line).unwrap();
            let envelope: EvidenceEvent =
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

fn normalized(evidence: &ReconstructedEvidence) -> Value {
    let observations: Vec<_> = evidence.observations().map(|value| json!({
        "id": value.id, "test_sequence": value.test_sequence, "observation_index": value.observation_index,
        "recorded_state": value.recorded_state.id, "capture": value.recorded_state.capture,
        "available_action_scope": value.available_action_scope,
        "widget_ids": value.recorded_state.widgets.iter().map(|widget| &widget.id).collect::<Vec<_>>(),
        "available_actions": value.available_actions,
        "artifact_ids": value.artifacts.iter().map(|artifact| &artifact.id).collect::<Vec<_>>(),
    })).collect();
    let status = |value: &HistoryStatus| match value {
        HistoryStatus::Running => "running",
        HistoryStatus::Finished { .. } => "finished",
    };
    json!({
        "status": "accepted",
        "counts": {
            "evidence_events": evidence.event_count(), "test_runs": evidence.test_run_count(),
            "test_sequences": evidence.test_sequence_count(), "observations": evidence.observation_count(),
            "action_descriptions": evidence.action_description_count(), "action_executions": evidence.action_execution_count(),
            "recorded_transitions": evidence.recorded_transition_count(), "oracle_results": evidence.oracle_results().len(),
            "artifact_references": evidence.observations().map(|value| value.artifacts.len()).sum::<usize>(),
            "recorded_states": evidence.recorded_state_count(),
        },
        "test_runs": evidence.test_runs().map(|value| json!({"id": value.id, "status": status(evidence.test_run_status(&value.id).unwrap())})).collect::<Vec<_>>(),
        "test_sequences": evidence.test_sequences().map(|value| json!({"id": value.id, "test_run": value.test_run, "sequence_index": value.sequence_index,
            "status": status(evidence.test_sequence_status(&value.id).unwrap())})).collect::<Vec<_>>(),
        "observations": observations,
        "action_descriptions": evidence.action_descriptions().map(|value| json!({"id": value.id, "action_type": value.action_type})).collect::<Vec<_>>(),
        "action_executions": evidence.action_executions().collect::<Vec<_>>(),
        "recorded_transitions": evidence.recorded_transitions(), "oracle_results": evidence.oracle_results(),
        "recorded_states": evidence.recorded_states().map(|value| &value.id).collect::<Vec<_>>(),
    })
}

fn normalized_error(error: &Error, event: &EventId) -> Value {
    json!({"code": error.code(), "event": event, "field": error.field().expect("conformance errors identify a field")})
}

fn assert_result(
    evidence: &ReconstructedEvidence,
    expected: &Value,
    failure: Option<Value>,
) {
    match expected["status"].as_str().unwrap() {
        "accepted" => {
            assert_eq!(failure, None);
            assert_eq!(normalized(evidence), *expected);
        }
        "rejected" => {
            assert_eq!(failure, Some(expected["error"].clone()));
            assert_eq!(normalized(evidence), expected["accepted_prefix"]);
        }
        status => panic!("Unsupported expected status: {status}"),
    }
}

fn assert_evidence_preserved(
    store: &JsonlStore,
    directory: &Path,
    source: &Path,
    accepted: &[EvidenceEvent],
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
            Event::ObservationRecorded(observation) => {
                assert_eq!(
                    store.evidence().observation(&observation.id),
                    Some(observation)
                );
                for artifact in &observation.artifacts {
                    assert_eq!(
                        store.read_artifact(artifact).unwrap(),
                        fs::read(source.join(&artifact.path)).unwrap()
                    );
                }
            }
            Event::TestRunStarted(run) => {
                assert_eq!(store.evidence().test_run(&run.id), Some(run))
            }
            Event::ActionDescribed(action) => {
                assert_eq!(
                    store.evidence().action_description(&action.id),
                    Some(action)
                )
            }
            Event::ActionSelected(execution) => assert_eq!(
                &store
                    .evidence()
                    .action_execution(&execution.id)
                    .unwrap()
                    .action_execution,
                execution
            ),
            Event::OracleResultRecorded(check) => {
                assert!(store.evidence().oracle_results().contains(check))
            }
            Event::TestRunFinished {
                test_run: run,
                reason,
            } => assert_eq!(
                store.evidence().test_run_status(run),
                Some(&HistoryStatus::Finished {
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
fn reads_shared_evidence_without_opening_a_writer() {
    for source in xtask::case_directories(support::specs_root()).unwrap() {
        let expected = read_json(&source.join("expected.json"));
        let original = fs::read(source.join("evidence.jsonl")).unwrap();
        match testar_evidence::EvidenceReader::read(&source) {
            Ok(evidence) => assert_result(&evidence, &expected, None),
            Err(error) => {
                let event =
                    EventId::new(expected["error"]["event"].as_str().unwrap())
                        .unwrap();
                assert_eq!(normalized_error(&error, &event), expected["error"]);
            }
        }
        assert_eq!(fs::read(source.join("evidence.jsonl")).unwrap(), original);
    }
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
                assert_result(store.evidence(), &expected, None);
                assert_evidence_preserved(
                    &store,
                    directory.path(),
                    &source,
                    &events,
                );
            }
            Err(reader_error) => {
                let mut evidence = ReconstructedEvidence::new(manifest).unwrap();
                let mut accepted = Vec::new();
                let mut failure = None;
                for envelope in events {
                    let before = normalized(&evidence);
                    match evidence.apply(envelope.clone()) {
                        Ok(_) => accepted.push(envelope),
                        Err(error) => {
                            assert_eq!(
                                normalized(&evidence),
                                before,
                                "Rejected event changed reconstructed evidence"
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
                assert_result(&evidence, &expected, failure);
                let prefix = tempdir().unwrap();
                copy_directory(&source, prefix.path());
                let mut evidence_log_bytes = Vec::new();
                for envelope in &accepted {
                    serde_json::to_writer(&mut evidence_log_bytes, envelope).unwrap();
                    evidence_log_bytes.push(b'\n');
                }
                fs::write(prefix.path().join("evidence.jsonl"), evidence_log_bytes)
                    .unwrap();
                let reopened = JsonlStore::open(prefix.path()).unwrap();
                assert_eq!(normalized(reopened.evidence()), normalized(&evidence));
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
            if let Event::ObservationRecorded(observation) = &envelope.event {
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
            let before = normalized(store.evidence());
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
                    assert_eq!(normalized(store.evidence()), before);
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
        assert_result(store.evidence(), &expected, failure.clone());
        assert_evidence_preserved(&store, directory.path(), &source, &accepted);
        drop(store);
        let reopened = JsonlStore::open(directory.path()).unwrap();
        assert_result(reopened.evidence(), &expected, failure);
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
