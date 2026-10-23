use serde_json::json;
use testar_evidence::*;

fn widget(id: &str, parent: Option<&str>) -> Widget {
    Widget {
        id: WidgetId::new(id).unwrap(),
        parent: parent.map(|id| WidgetId::new(id).unwrap()),
        properties: PropertyMap::new(),
    }
}

fn concrete_state(widgets: Vec<Widget>) -> RecordedState {
    RecordedState {
        id: RecordedStateId::new("recorded_state-1").unwrap(),
        properties: PropertyMap::new(),
        widgets,
        capture: CaptureScope::Complete,
    }
}

fn observation() -> Observation {
    Observation {
        id: ObservationId::new("observation-1").unwrap(),
        test_sequence: TestSequenceId::new("run-1").unwrap(),
        observation_index: 0,
        timestamp_micros: 1_000,
        recorded_state: concrete_state(vec![
            widget("window", None),
            widget("button", Some("window")),
        ]),
        available_actions: Vec::new(),
        available_action_scope: AvailableActionScope::Unknown,
        artifacts: Vec::new(),
    }
}

fn artifact(path: &str) -> ArtifactReference {
    ArtifactReference {
        id: "screenshot-1".into(),
        path: path.into(),
        media_type: "image/png".into(),
        sha256: "ab".repeat(32),
        width: Some(640),
        height: Some(480),
    }
}

#[test]
fn typed_ids_reject_blank_values_through_construction_and_deserialization() {
    assert!(RecordedStateId::new("").is_err());
    assert!(RecordedStateId::new(" \r\n\t").is_err());
    assert!(serde_json::from_str::<RecordedStateId>("\"   \"").is_err());
    assert!(serde_json::from_str::<RecordedStateId>("42").is_err());

    let id = RecordedStateId::new("native:main-screen").unwrap();
    let encoded = serde_json::to_string(&id).unwrap();
    let decoded: RecordedStateId = serde_json::from_str(&encoded).unwrap();
    assert_eq!(decoded, id);
    assert_eq!(decoded.as_str(), "native:main-screen");
}

#[test]
fn nested_id_deserialization_does_not_bypass_validation() {
    let mut encoded = serde_json::to_value(observation()).unwrap();
    encoded["recorded_state"]["widgets"][0]["id"] = json!(" ");
    assert!(serde_json::from_value::<Observation>(encoded).is_err());
}

#[test]
fn available_action_scope_is_required_and_preserved() {
    let mut encoded = serde_json::to_value(observation()).unwrap();
    encoded["available_action_scope"] = json!("eligible");
    let decoded: Observation = serde_json::from_value(encoded.clone()).unwrap();
    assert_eq!(
        decoded.available_action_scope,
        AvailableActionScope::Eligible
    );
    encoded
        .as_object_mut()
        .unwrap()
        .remove("available_action_scope");
    assert!(serde_json::from_value::<Observation>(encoded).is_err());
}

#[test]
fn observed_properties_preserve_structured_values_and_explicit_null() {
    let mut observed = observation();
    observed
        .recorded_state
        .properties
        .insert("sut:mode".into(), json!("main"));
    observed
        .recorded_state
        .properties
        .insert("sut:empty".into(), json!(null));
    observed.recorded_state.widgets[1]
        .properties
        .insert("ui:enabled".into(), json!(true));
    observed.recorded_state.widgets[1].properties.insert(
        "ui:geometry".into(),
        json!({"position": [5, 10], "size": {"width": 100, "height": 30}}),
    );
    observed.validate().unwrap();

    let encoded = serde_json::to_vec(&observed).unwrap();
    let decoded: Observation = serde_json::from_slice(&encoded).unwrap();
    decoded.validate().unwrap();
    assert_eq!(decoded, observed);
    assert_eq!(
        decoded.recorded_state.properties.get("sut:empty"),
        Some(&json!(null))
    );
    assert!(
        !decoded
            .recorded_state
            .properties
            .contains_key("sut:unobserved")
    );
}

#[test]
fn generic_widget_forests_allow_multiple_roots_and_preserve_order() {
    let state = concrete_state(vec![
        widget("window-a", None),
        widget("first", Some("window-a")),
        widget("second", Some("window-a")),
        widget("window-b", None),
    ]);
    state.validate().unwrap();

    let decoded: RecordedState =
        serde_json::from_value(serde_json::to_value(&state).unwrap()).unwrap();
    assert_eq!(decoded.widgets, state.widgets);
}

#[test]
fn hierarchy_rejects_duplicate_widgets_and_missing_parents() {
    let repeated =
        concrete_state(vec![widget("window", None), widget("window", None)]);
    assert!(matches!(repeated.validate(), Err(Error::Conflict(_))));

    let missing = concrete_state(vec![widget("button", Some("absent"))]);
    assert!(matches!(
        missing.validate(),
        Err(Error::UnknownReference(_))
    ));
}

#[test]
fn hierarchy_rejects_self_parenting_and_indirect_cycles() {
    let self_parent = concrete_state(vec![widget("window", Some("window"))]);
    assert!(matches!(
        self_parent.validate(),
        Err(Error::InvalidRecord(_))
    ));

    let cycle = concrete_state(vec![
        widget("first", Some("second")),
        widget("second", Some("third")),
        widget("third", Some("first")),
    ]);
    assert!(matches!(cycle.validate(), Err(Error::InvalidRecord(_))));
}

#[test]
fn unavailable_capture_is_distinct_from_a_captured_empty_state() {
    let mut state = concrete_state(Vec::new());
    state.validate().unwrap();
    state.capture = CaptureScope::Unavailable;
    state.validate().unwrap();
    state.widgets.push(widget("unexpected", None));
    assert!(matches!(state.validate(), Err(Error::InvalidRecord(_))));
}

#[test]
fn observation_available_actions_bind_to_local_widgets_or_global_actions() {
    let mut observed = observation();
    observed.available_actions = vec![
        AvailableAction {
            action_description: ActionDescriptionId::new("activate").unwrap(),
            target_widget: Some(WidgetId::new("button").unwrap()),
        },
        AvailableAction {
            action_description: ActionDescriptionId::new("restart").unwrap(),
            target_widget: None,
        },
    ];
    observed.validate().unwrap();

    observed.available_actions[0].target_widget =
        Some(WidgetId::new("another-observation-widget").unwrap());
    assert!(matches!(
        observed.validate(),
        Err(Error::UnknownReference(_))
    ));
}

#[test]
fn duplicate_available_actions_do_not_inflate_a_single_observation() {
    let mut observed = observation();
    let candidate = AvailableAction {
        action_description: ActionDescriptionId::new("activate").unwrap(),
        target_widget: Some(WidgetId::new("button").unwrap()),
    };
    observed.available_actions = vec![candidate.clone(), candidate];
    assert!(matches!(observed.validate(), Err(Error::Conflict(_))));
}

#[test]
fn distinct_observations_preserve_their_own_concrete_data() {
    let first = observation();
    let mut second = first.clone();
    second.id = ObservationId::new("observation-2").unwrap();
    second.observation_index = 1;
    second.recorded_state.id =
        RecordedStateId::new("recorded_state-2").unwrap();
    second.recorded_state.widgets[1]
        .properties
        .insert("ui:text".into(), json!("changed"));
    first.validate().unwrap();
    second.validate().unwrap();

    assert_ne!(first.id, second.id);
    assert_ne!(first.recorded_state, second.recorded_state);
}

#[test]
fn descriptive_property_names_are_validated() {
    let mut state = concrete_state(Vec::new());
    state.properties.insert(" ".into(), json!(false));
    assert!(matches!(state.validate(), Err(Error::InvalidRecord(_))));
    state.properties.clear();
    state.properties.insert("ui:\nrole".into(), json!("button"));
    assert!(matches!(state.validate(), Err(Error::InvalidRecord(_))));
}

#[test]
fn artifact_paths_stay_relative_and_portable_on_all_hosts() {
    artifact("artifacts/image.png").validate().unwrap();
    for path in [
        "../outside.png",
        "artifacts/../outside.png",
        "/absolute.png",
        "C:/absolute.png",
        "C:drive-relative.png",
        "artifacts\\image.png",
        "//server/share/image.png",
        "artifacts//image.png",
        "artifacts/./image.png",
        "artifacts/.. /image.png",
        "artifacts/CON.png",
        "artifacts/lpt1.txt",
        "artifacts/image.png:stream",
        "artifacts/image\n.png",
    ] {
        assert!(
            artifact(path).validate().is_err(),
            "accepted unsafe path {path:?}"
        );
    }
}

#[test]
fn artifacts_require_digest_and_valid_optional_dimensions() {
    let mut reference = artifact("artifacts/image.png");
    reference.sha256 = "ab".repeat(31);
    assert!(reference.validate().is_err());
    reference.sha256 = "z".repeat(64);
    assert!(reference.validate().is_err());
    reference.sha256 = "AB".repeat(32);
    assert!(reference.validate().is_err());
    reference.sha256 = "ab".repeat(32);
    reference.width = Some(0);
    reference.validate().unwrap();
    reference.width = None;
    reference.height = None;
    reference.validate().unwrap();
}

#[test]
fn duplicate_artifact_identity_in_an_observation_is_rejected() {
    let mut observed = observation();
    let reference = artifact("artifacts/image.png");
    observed.artifacts = vec![reference.clone(), reference];
    assert!(matches!(observed.validate(), Err(Error::Conflict(_))));
}

#[test]
fn oracle_results_retain_their_explicit_scope_and_uncertainty() {
    let result = OracleResult {
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
    result.validate().unwrap();
    let encoded = serde_json::to_value(&result).unwrap();
    assert_eq!(encoded["oracle_status"], json!("pending"));
    let decoded: OracleResult = serde_json::from_value(encoded).unwrap();
    assert_eq!(decoded, result);
    assert_ne!(decoded.oracle_status, OracleStatus::Passed);
}
