use serde_json::{Value, json};
use testar_evidence::*;

fn apply(
    evidence: &mut ReconstructedEvidence,
    id: &str,
    event_type: &str,
    data: Value,
) -> Result<bool> {
    evidence.apply(
        serde_json::from_value(json!({"schema_version": 3, "id": id,
        "event": {"event_type": event_type, "data": data}}))
        .unwrap(),
    )
}

fn started() -> ReconstructedEvidence {
    let mut evidence = ReconstructedEvidence::new(Manifest {
        schema_version: 3,
        evidence_id: EvidenceId::new("unit:evidence").unwrap(),
    })
    .unwrap();
    apply(&mut evidence, "start", "test_run_started", json!({"id": "run", "producer": "unit", "source_run": "1", "properties": {}})).unwrap();
    evidence
}

fn sequence(evidence: &mut ReconstructedEvidence, id: &str, index: u64) {
    apply(
        evidence,
        &format!("start-{id}"),
        "test_sequence_started",
        json!({"id": id, "test_run": "run",
        "sequence_index": index, "source_sequence": id, "properties": {}}),
    )
    .unwrap();
}

fn observation(sequence: &str, id: &str, index: u64) -> Value {
    json!({"id": id, "test_sequence": sequence, "observation_index": index, "timestamp_micros": index,
        "recorded_state": {"id": "shared-state", "properties": {"ui.title": "Accounts"}, "widgets": [], "capture": "complete"},
        "available_action_scope": "eligible", "available_actions": [{"action_description": "refresh", "target_widget": null}], "artifacts": []})
}

#[test]
fn interleaved_sequences_keep_independent_executions_and_transition_recording_order()
 {
    let mut evidence = started();
    sequence(&mut evidence, "first", 1);
    sequence(&mut evidence, "second", 2);
    apply(
        &mut evidence,
        "describe",
        "action_described",
        json!({"id": "refresh", "action_type": "refresh", "properties": {}}),
    )
    .unwrap();
    for (seq, obs, execution) in [
        ("first", "a", "execution-a"),
        ("second", "b", "execution-b"),
    ] {
        apply(
            &mut evidence,
            obs,
            "observation_recorded",
            observation(seq, obs, 1),
        )
        .unwrap();
        apply(&mut evidence, execution, "action_selected", json!({"id": execution, "test_sequence": seq,
            "source_observation": obs, "action_description": "refresh", "target_widget": null, "parameters": {}})).unwrap();
        apply(&mut evidence, &format!("return-{execution}"), "action_execution_reported", json!({"action_execution": execution, "outcome": "returned", "error": null})).unwrap();
    }
    for (seq, obs, execution) in [
        ("second", "b-result", "execution-b"),
        ("first", "a-result", "execution-a"),
    ] {
        apply(
            &mut evidence,
            obs,
            "observation_recorded",
            observation(seq, obs, 2),
        )
        .unwrap();
        apply(&mut evidence, &format!("link-{execution}"), "transition_recorded", json!({"action_execution": execution, "resulting_observation": obs})).unwrap();
    }
    assert_eq!(evidence.recorded_state_count(), 1);
    assert_eq!(evidence.observation_count(), 4);
    assert_eq!(
        evidence
            .action_executions()
            .map(|value| value.action_execution.id.as_str())
            .collect::<Vec<_>>(),
        ["execution-a", "execution-b"]
    );
    assert_eq!(
        evidence
            .recorded_transitions()
            .iter()
            .map(|value| value.action_execution.as_str())
            .collect::<Vec<_>>(),
        ["execution-b", "execution-a"]
    );
    let count = evidence.event_count();
    let failure = apply(
        &mut evidence,
        "early-finish",
        "test_run_finished",
        json!({"test_run": "run", "reason": "completed"}),
    )
    .unwrap_err();
    assert_eq!(failure.code(), "invalid_lifecycle");
    assert_eq!(evidence.event_count(), count);
    for seq in ["first", "second"] {
        apply(
            &mut evidence,
            &format!("finish-{seq}"),
            "test_sequence_finished",
            json!({"test_sequence": seq, "reason": "completed"}),
        )
        .unwrap();
    }
    apply(
        &mut evidence,
        "finish",
        "test_run_finished",
        json!({"test_run": "run", "reason": "completed"}),
    )
    .unwrap();
}

#[test]
fn severity_is_optional_source_defined_and_independent_of_status() {
    let mut evidence = started();
    let base = json!({"test_run": "run", "name": "Condition holds", "oracle": {"id": "condition"},
        "oracle_target": {"target_type": "test_run"}, "oracle_status": "failed", "oracle_details": {}});
    apply(
        &mut evidence,
        "omitted",
        "oracle_result_recorded",
        base.clone(),
    )
    .unwrap();
    for (index, severity) in [
        json!({"label": " warning ", "value": -12}),
        json!({"label": "Priorité équipe"}),
    ]
    .into_iter()
    .enumerate()
    {
        let mut data = base.clone();
        data["oracle_severity"] = severity;
        apply(
            &mut evidence,
            &format!("severity-{index}"),
            "oracle_result_recorded",
            data,
        )
        .unwrap();
    }
    assert!(evidence.oracle_results()[0].oracle_severity.is_none());
    assert_eq!(
        evidence.oracle_results()[1]
            .oracle_severity
            .as_ref()
            .unwrap()
            .label,
        " warning "
    );
    assert!(
        evidence
            .oracle_results()
            .iter()
            .all(|result| result.oracle_status == OracleStatus::Failed)
    );
    let mut invalid = base;
    invalid["oracle_severity"] = json!({"label": " "});
    let error = apply(&mut evidence, "invalid", "oracle_result_recorded", invalid)
        .unwrap_err();
    assert_eq!(
        (error.code(), error.field()),
        ("invalid_oracle_severity", Some("oracle_severity.label"))
    );
    assert_eq!(evidence.oracle_results().len(), 3);
}

#[test]
fn structural_fields_are_strict_while_source_json_preserves_exact_values() {
    let mut available =
        json!({"action_description": "refresh", "target_widget": null});
    assert!(
        serde_json::from_value::<AvailableAction>(available.clone()).is_ok()
    );
    available.as_object_mut().unwrap().remove("target_widget");
    assert!(serde_json::from_value::<AvailableAction>(available).is_err());
    let exact = "340282366920938463463374607431768211455";
    let mut state = observation("sequence", "observation", u64::MAX);
    state["recorded_state"]["properties"]["kind"] = json!("source-defined key");
    state["recorded_state"]["properties"]["large"] =
        serde_json::from_str(exact).unwrap();
    let decoded: Observation = serde_json::from_value(state.clone()).unwrap();
    assert_eq!(serde_json::to_value(&decoded).unwrap(), state);
    assert_eq!(
        decoded.recorded_state.properties["large"].to_string(),
        exact
    );
    state["extra"] = json!(true);
    assert!(serde_json::from_value::<Observation>(state).is_err());
}
