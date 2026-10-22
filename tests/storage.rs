use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::Path;

use serde_json::json;
use tempfile::tempdir;
use testar_evidence::{
    ActionCandidate, ActionDefinition, ActionId, ArtifactReference,
    CandidateScope, CaptureScope, ConcreteState, ConcreteStateId, Error, Event,
    EventId, EvidenceEvent, Execution, ExecutionId, JsonlStore, Manifest,
    ModelId, Observation, ObservationId, PropertyMap, Run, RunId,
    SCHEMA_VERSION, Widget, WidgetId,
};

fn manifest() -> Manifest {
    Manifest {
        schema_version: SCHEMA_VERSION,
        model: ModelId::new("local-model").unwrap(),
    }
}

fn run() -> Run {
    Run {
        id: RunId::new("producer:invocation:run").unwrap(),
        producer: "test-producer".into(),
        source_run: "0".into(),
        properties: PropertyMap::new(),
    }
}

fn action() -> ActionDefinition {
    ActionDefinition {
        id: ActionId::new("activate").unwrap(),
        kind: "activate".into(),
        properties: PropertyMap::new(),
    }
}

fn observation(name: &str, sequence: u64) -> Observation {
    Observation {
        id: ObservationId::new(name).unwrap(),
        run: run().id,
        sequence,
        timestamp_micros: sequence * 1_000,
        state: ConcreteState {
            id: ConcreteStateId::new(format!("concrete-{name}")).unwrap(),
            properties: PropertyMap::from([("app:page".into(), json!(name))]),
            widgets: vec![Widget {
                id: WidgetId::new("component").unwrap(),
                parent: None,
                properties: PropertyMap::from([(
                    "ui:role".into(),
                    json!("button"),
                )]),
            }],
            capture: CaptureScope::Complete,
        },
        candidate_scope: CandidateScope::Generated,
        candidates: vec![ActionCandidate {
            action: action().id,
            target: Some(WidgetId::new("component").unwrap()),
        }],
        artifacts: Vec::new(),
    }
}

fn execution() -> Execution {
    Execution {
        id: ExecutionId::new("execution").unwrap(),
        run: run().id,
        source: ObservationId::new("first").unwrap(),
        action: action().id,
        target: Some(WidgetId::new("component").unwrap()),
        parameters: PropertyMap::new(),
    }
}

fn event(name: &str, event: Event) -> EvidenceEvent {
    EvidenceEvent {
        schema_version: SCHEMA_VERSION,
        id: EventId::new(name).unwrap(),
        event,
    }
}

fn start(store: &mut JsonlStore) {
    store
        .record(event("run", Event::RunStarted(run())))
        .unwrap();
    store
        .record(event("action", Event::ActionDefined(action())))
        .unwrap();
}

fn append(path: &Path, bytes: &[u8]) {
    let mut file = OpenOptions::new().append(true).open(path).unwrap();
    file.write_all(bytes).unwrap();
    file.sync_all().unwrap();
}

#[test]
fn saves_reopens_and_continues_without_duplicating_evidence() {
    let directory = tempdir().unwrap();
    let mut store = JsonlStore::create(directory.path(), manifest()).unwrap();
    start(&mut store);
    store
        .record(event("first", Event::Observed(observation("first", 0))))
        .unwrap();
    store
        .record(event("selected", Event::ActionSelected(execution())))
        .unwrap();
    store
        .record(event(
            "submitted",
            Event::ActionSubmitted {
                execution: execution().id,
                accepted: true,
                error: None,
            },
        ))
        .unwrap();
    store
        .record(event("second", Event::Observed(observation("second", 1))))
        .unwrap();
    store
        .record(event(
            "result",
            Event::ActionResult {
                execution: execution().id,
                observation: ObservationId::new("second").unwrap(),
            },
        ))
        .unwrap();
    assert_eq!(store.model().link_count(), 1);
    drop(store);

    let mut store = JsonlStore::open(directory.path()).unwrap();
    assert_eq!(store.model().manifest(), &manifest());
    assert_eq!(store.model().observation_count(), 2);
    assert_eq!(store.model().link_count(), 1);
    assert_eq!(store.model().event_count(), 7);
    store
        .record(event(
            "finish",
            Event::RunFinished {
                run: run().id,
                reason: "complete".into(),
            },
        ))
        .unwrap();
    store.flush().unwrap();
    drop(store);

    let store = JsonlStore::open(directory.path()).unwrap();
    assert_eq!(store.model().event_count(), 8);
    assert_eq!(store.model().link_count(), 1);
}

#[test]
fn writer_lock_excludes_open_create_and_recovery_until_drop() {
    let directory = tempdir().unwrap();
    let store = JsonlStore::create(directory.path(), manifest()).unwrap();
    assert!(JsonlStore::open(directory.path()).is_err());
    assert!(JsonlStore::create(directory.path(), manifest()).is_err());
    assert!(JsonlStore::recover_tail(directory.path()).is_err());
    drop(store);

    assert!(JsonlStore::open(directory.path()).is_ok());
    assert!(matches!(
        JsonlStore::create(directory.path(), manifest()),
        Err(Error::Conflict(_))
    ));
}

#[test]
fn screenshot_paths_have_extensions_and_deduplicate_unchanged_bytes() {
    let directory = tempdir().unwrap();
    let mut store = JsonlStore::create(directory.path(), manifest()).unwrap();
    let bytes = include_bytes!("fixtures/screenshot.png");
    let first = store
        .put_artifact(bytes, "image/png", Some(1), Some(1))
        .unwrap();
    let second = store
        .put_artifact(bytes, "image/png", Some(1), Some(1))
        .unwrap();
    assert_eq!(first, second);
    assert_eq!(first.path, format!("artifacts/{}.png", first.sha256));
    assert_eq!(store.read_artifact(&first).unwrap(), bytes);
    assert_eq!(
        fs::read_dir(directory.path().join("artifacts"))
            .unwrap()
            .count(),
        1
    );
    let binary = store
        .put_artifact(b"generic bytes", "application/octet-stream", None, None)
        .unwrap();
    assert!(binary.path.ends_with(".bin"));
    assert_eq!(store.read_artifact(&binary).unwrap(), b"generic bytes");
}

#[test]
fn exact_duplicates_are_idempotent_and_conflicting_ids_do_not_append() {
    let directory = tempdir().unwrap();
    let log = directory.path().join("evidence.jsonl");
    let mut store = JsonlStore::create(directory.path(), manifest()).unwrap();
    let original = event("run", Event::RunStarted(run()));
    assert!(store.record(original.clone()).unwrap());
    let original_length = fs::metadata(&log).unwrap().len();
    assert!(!store.record(original.clone()).unwrap());
    assert_eq!(fs::metadata(&log).unwrap().len(), original_length);

    let mut conflicting = run();
    conflicting.producer = "another-producer".into();
    assert!(matches!(
        store.record(event("run", Event::RunStarted(conflicting))),
        Err(Error::Conflict(_))
    ));
    assert_eq!(fs::metadata(&log).unwrap().len(), original_length);
    drop(store);

    let mut repeated = serde_json::to_vec(&original).unwrap();
    repeated.push(b'\n');
    append(&log, &repeated);
    let store = JsonlStore::open(directory.path()).unwrap();
    assert_eq!(store.model().run_count(), 1);
    assert_eq!(store.model().event_count(), 1);
}

#[test]
fn incomplete_tail_requires_explicit_recovery_before_continuing() {
    let directory = tempdir().unwrap();
    let log = directory.path().join("evidence.jsonl");
    let mut store = JsonlStore::create(directory.path(), manifest()).unwrap();
    store
        .record(event("run", Event::RunStarted(run())))
        .unwrap();
    drop(store);

    let valid_length = fs::metadata(&log).unwrap().len();
    let fragment = b"{\"schema_version\":1,\"id\":";
    append(&log, fragment);
    let error = JsonlStore::open(directory.path()).err().unwrap();
    assert!(error.to_string().contains("incomplete final fragment"));
    assert_eq!(
        fs::metadata(&log).unwrap().len(),
        valid_length + fragment.len() as u64
    );
    assert_eq!(
        JsonlStore::recover_tail(directory.path()).unwrap(),
        fragment.len() as u64
    );
    assert_eq!(fs::metadata(&log).unwrap().len(), valid_length);
    assert_eq!(JsonlStore::recover_tail(directory.path()).unwrap(), 0);

    let mut store = JsonlStore::open(directory.path()).unwrap();
    assert_eq!(store.model().run_count(), 1);
    store
        .record(event("action", Event::ActionDefined(action())))
        .unwrap();
    assert_eq!(store.model().event_count(), 2);
}

#[test]
fn completed_corruption_is_never_skipped_or_truncated_by_tail_recovery() {
    let directory = tempdir().unwrap();
    let log = directory.path().join("evidence.jsonl");
    let mut store = JsonlStore::create(directory.path(), manifest()).unwrap();
    store
        .record(event("run", Event::RunStarted(run())))
        .unwrap();
    drop(store);
    append(&log, b"not valid JSON\n{unfinished");
    let original = fs::read(&log).unwrap();

    let error = JsonlStore::open(directory.path()).err().unwrap();
    assert!(error.to_string().contains("line 2"));
    assert!(JsonlStore::recover_tail(directory.path()).is_err());
    assert_eq!(fs::read(&log).unwrap(), original);
}

#[test]
fn rejects_unsupported_manifest_and_event_versions_without_mutation() {
    let directory = tempdir().unwrap();
    let mut unsupported = manifest();
    unsupported.schema_version += 1;
    assert!(JsonlStore::create(directory.path(), unsupported.clone()).is_err());
    assert!(!directory.path().join("manifest.json").exists());

    let mut store = JsonlStore::create(directory.path(), manifest()).unwrap();
    let mut envelope = event("run", Event::RunStarted(run()));
    envelope.schema_version += 1;
    assert!(store.record(envelope.clone()).is_err());
    assert_eq!(
        fs::metadata(directory.path().join("evidence.jsonl"))
            .unwrap()
            .len(),
        0
    );
    drop(store);

    fs::write(
        directory.path().join("manifest.json"),
        serde_json::to_vec(&unsupported).unwrap(),
    )
    .unwrap();
    assert!(JsonlStore::open(directory.path()).is_err());
    fs::write(
        directory.path().join("manifest.json"),
        serde_json::to_vec(&manifest()).unwrap(),
    )
    .unwrap();
    let mut bytes = serde_json::to_vec(&envelope).unwrap();
    bytes.push(b'\n');
    append(&directory.path().join("evidence.jsonl"), &bytes);
    let error = JsonlStore::open(directory.path()).err().unwrap();
    assert!(error.to_string().contains("line 1"));
    assert!(error.to_string().contains("unsupported schema version"));
}

#[test]
fn rejects_unknown_references_before_append_and_during_replay() {
    let directory = tempdir().unwrap();
    let log = directory.path().join("evidence.jsonl");
    let mut store = JsonlStore::create(directory.path(), manifest()).unwrap();
    let envelope =
        event("observation", Event::Observed(observation("first", 0)));
    assert!(matches!(
        store.record(envelope.clone()),
        Err(Error::UnknownReference(_))
    ));
    assert_eq!(fs::metadata(&log).unwrap().len(), 0);
    drop(store);

    let mut record = serde_json::to_vec(&envelope).unwrap();
    record.push(b'\n');
    append(&log, &record);
    let error = JsonlStore::open(directory.path()).err().unwrap();
    assert!(matches!(error, Error::UnknownReference(_)));
    assert!(error.to_string().contains("line 1"));
}

#[test]
fn referenced_artifacts_reopen_after_copying_the_model_directory() {
    let source = tempdir().unwrap();
    let destination = tempdir().unwrap();
    let bytes = b"portable visual evidence";
    let mut store = JsonlStore::create(source.path(), manifest()).unwrap();
    start(&mut store);
    let reference = store
        .put_artifact(bytes, "image/png", Some(8), Some(4))
        .unwrap();
    assert_eq!(store.read_artifact(&reference).unwrap(), bytes);
    assert_eq!(
        store
            .put_artifact(bytes, "image/png", Some(8), Some(4))
            .unwrap(),
        reference
    );
    let mut observed = observation("first", 0);
    observed.artifacts.push(reference.clone());
    store
        .record(event("first", Event::Observed(observed)))
        .unwrap();
    drop(store);

    for name in ["manifest.json", "evidence.jsonl"] {
        fs::copy(source.path().join(name), destination.path().join(name))
            .unwrap();
    }
    fs::create_dir(destination.path().join("artifacts")).unwrap();
    fs::copy(
        source.path().join(&reference.path),
        destination.path().join(&reference.path),
    )
    .unwrap();
    let store = JsonlStore::open(destination.path()).unwrap();
    assert_eq!(store.model().observation_count(), 1);
    assert_eq!(store.read_artifact(&reference).unwrap(), bytes);
}

#[test]
fn missing_or_corrupted_artifacts_are_not_acknowledged() {
    let directory = tempdir().unwrap();
    let mut store = JsonlStore::create(directory.path(), manifest()).unwrap();
    start(&mut store);
    let mut observed = observation("first", 0);
    let missing = ArtifactReference {
        id: "missing".into(),
        path: format!("artifacts/{}", "a".repeat(64)),
        media_type: "image/png".into(),
        sha256: "a".repeat(64),
        width: None,
        height: None,
    };
    observed.artifacts.push(missing);
    assert!(
        store
            .record(event("missing", Event::Observed(observed)))
            .is_err()
    );
    assert_eq!(store.model().observation_count(), 0);

    let reference = store
        .put_artifact(b"original bytes", "image/png", None, None)
        .unwrap();
    let mut observed = observation("first", 0);
    observed.artifacts.push(reference.clone());
    store
        .record(event("first", Event::Observed(observed)))
        .unwrap();
    fs::write(directory.path().join(&reference.path), b"modified bytes")
        .unwrap();
    assert!(matches!(
        store.read_artifact(&reference),
        Err(Error::Conflict(_))
    ));
    assert!(matches!(
        store.put_artifact(b"original bytes", "image/png", None, None),
        Err(Error::Conflict(_))
    ));
    assert_eq!(
        fs::read(directory.path().join(&reference.path)).unwrap(),
        b"modified bytes"
    );
    drop(store);
    let error = JsonlStore::open(directory.path()).err().unwrap();
    assert!(error.to_string().contains("artifact digest mismatch"));
    assert!(error.to_string().contains("line 3"));
}

#[test]
fn rejects_invalid_artifact_metadata_and_traversal() {
    let directory = tempdir().unwrap();
    let mut store = JsonlStore::create(directory.path(), manifest()).unwrap();
    assert!(store.put_artifact(b"bytes", " ", None, None).is_err());
    assert!(
        store
            .put_artifact(b"bytes", "image/png", Some(0), Some(1))
            .is_err()
    );
    let mut reference = store
        .put_artifact(b"bytes", "image/png", None, None)
        .unwrap();
    reference.path = "../outside".into();
    assert!(matches!(
        store.read_artifact(&reference),
        Err(Error::InvalidRecord(_))
    ));
}

#[cfg(unix)]
#[test]
fn rejects_artifact_symlinks_that_escape_the_model_directory() {
    use std::os::unix::fs::symlink;

    let directory = tempdir().unwrap();
    let outside = tempdir().unwrap();
    let mut store = JsonlStore::create(directory.path(), manifest()).unwrap();
    fs::write(outside.path().join("image"), b"outside").unwrap();
    symlink(outside.path(), directory.path().join("artifacts")).unwrap();
    assert!(matches!(
        store.put_artifact(b"outside", "image/png", None, None),
        Err(Error::InvalidRecord(_))
    ));
    let reference = ArtifactReference {
        id: "outside".into(),
        path: "artifacts/image".into(),
        media_type: "image/png".into(),
        sha256: "a".repeat(64),
        width: None,
        height: None,
    };
    assert!(matches!(
        store.read_artifact(&reference),
        Err(Error::InvalidRecord(_))
    ));
}
