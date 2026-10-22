mod support;

use serde_json::json;
use tempfile::tempdir;
use testar_evidence::*;

fn shared_evidence() -> String {
    std::fs::read_to_string(
        support::specs_root()
            .join("conformance/cases/initial-observation/evidence.jsonl"),
    )
    .unwrap()
}

fn manifest() -> Manifest {
    Manifest {
        schema_version: SCHEMA_VERSION,
        model: ModelId::new("example").unwrap(),
    }
}

#[test]
fn shared_evidence_replays_into_the_state_model() {
    let mut model = StateModel::new(manifest()).unwrap();
    for line in shared_evidence().lines() {
        let envelope: EventEnvelope = serde_json::from_str(line).unwrap();
        assert!(model.apply(envelope).unwrap());
    }
    assert_eq!(model.event_count(), 3);
    assert_eq!(model.concrete_state_count(), 1);
    assert_eq!(model.link_count(), 0);
    assert_eq!(model.action_count(), 1);
}

#[test]
fn floating_point_evidence_reopens_exactly_and_retry_stays_idempotent() {
    let directory = tempdir().unwrap();
    let mut store = JsonlStore::create(directory.path(), manifest()).unwrap();
    let started: EventEnvelope =
        serde_json::from_str(shared_evidence().lines().next().unwrap())
            .unwrap();
    store.record(started).unwrap();
    let observation = Observation {
        id: ObservationId::new("measurement-1").unwrap(),
        run: RunId::new("example:run:1").unwrap(),
        sequence: 1,
        timestamp_micros: 1,
        state: ConcreteState {
            id: ConcreteStateId::new("measurement-snapshot").unwrap(),
            properties: PropertyMap::from([(
                "measurements".into(),
                json!([
                    51.248178375505404_f64,
                    2.0030397744267762e-253_f64,
                    -0.0_f64,
                ]),
            )]),
            widgets: vec![],
            capture: CaptureScope::Complete,
        },
        candidate_scope: CandidateScope::Unknown,
        candidates: vec![],
        artifacts: vec![],
    };
    let envelope = EventEnvelope {
        schema_version: SCHEMA_VERSION,
        id: EventId::new("measurement-event").unwrap(),
        event: Event::Observed(observation.clone()),
    };
    store.record(envelope.clone()).unwrap();
    drop(store);
    let mut reopened = JsonlStore::open(directory.path()).unwrap();
    let actual = reopened.model().observation(&observation.id).unwrap();
    assert_eq!(actual, &observation);
    assert!(!reopened.record(envelope).unwrap());
}

#[test]
fn manifest_and_observation_reject_unknown_fields() {
    let mut encoded = serde_json::to_value(manifest()).unwrap();
    encoded["unexpected_field"] = json!("unexpected-value");
    let error = serde_json::from_value::<Manifest>(encoded).unwrap_err();
    assert!(error.to_string().contains("unexpected_field"));
    let mut observed: serde_json::Value =
        serde_json::from_str(shared_evidence().lines().last().unwrap())
            .unwrap();
    observed["event"]["data"]["unexpected_field"] = json!("unexpected-value");
    let error = serde_json::from_value::<EventEnvelope>(observed).unwrap_err();
    assert!(error.to_string().contains("unexpected_field"));
}

#[test]
fn opening_data_with_unknown_fields_preserves_stored_files() {
    for unknown_field_in_manifest in [true, false] {
        let directory = tempdir().unwrap();
        let mut encoded = serde_json::to_value(manifest()).unwrap();
        let mut records: Vec<serde_json::Value> = shared_evidence()
            .lines()
            .map(|line| serde_json::from_str(line).unwrap())
            .collect();
        if unknown_field_in_manifest {
            encoded["unexpected_field"] = json!("unexpected-value");
        } else {
            records.last_mut().unwrap()["event"]["data"]["unexpected_field"] =
                json!("unexpected-value");
        }
        let manifest_bytes = serde_json::to_vec(&encoded).unwrap();
        let evidence = records
            .iter()
            .map(|record| serde_json::to_string(record).unwrap() + "\n")
            .collect::<String>();
        let manifest_path = directory.path().join("manifest.json");
        let evidence_path = directory.path().join("evidence.jsonl");
        std::fs::write(&manifest_path, &manifest_bytes).unwrap();
        std::fs::write(&evidence_path, &evidence).unwrap();
        assert!(JsonlStore::open(directory.path()).is_err());
        assert_eq!(std::fs::read(manifest_path).unwrap(), manifest_bytes);
        assert_eq!(std::fs::read_to_string(evidence_path).unwrap(), evidence);
    }
}
