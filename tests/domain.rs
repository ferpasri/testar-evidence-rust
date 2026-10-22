use serde_json::json;
use testar_evidence::{
    ActionCandidate, ActionId, ArtifactReference, CandidateScope, CaptureScope,
    CheckResult, CheckRule, CheckStatus, CheckTarget, ConcreteState,
    ConcreteStateId, Error, ExpectedValue, Observation, ObservationId,
    PropertyMap, RunId, Widget, WidgetId,
};

fn widget(id: &str, parent: Option<&str>) -> Widget {
    Widget {
        id: WidgetId::new(id).unwrap(),
        parent: parent.map(|id| WidgetId::new(id).unwrap()),
        properties: PropertyMap::new(),
    }
}

fn concrete_state(widgets: Vec<Widget>) -> ConcreteState {
    ConcreteState {
        id: ConcreteStateId::new("snapshot-1").unwrap(),
        properties: PropertyMap::new(),
        widgets,
        capture: CaptureScope::Complete,
    }
}

fn observation() -> Observation {
    Observation {
        id: ObservationId::new("observation-1").unwrap(),
        run: RunId::new("run-1").unwrap(),
        sequence: 0,
        timestamp_micros: 1_000,
        state: concrete_state(vec![
            widget("window", None),
            widget("button", Some("window")),
        ]),
        candidates: Vec::new(),
        candidate_scope: CandidateScope::Unknown,
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
    assert!(ConcreteStateId::new("").is_err());
    assert!(ConcreteStateId::new(" \r\n\t").is_err());
    assert!(serde_json::from_str::<ConcreteStateId>("\"   \"").is_err());
    assert!(serde_json::from_str::<ConcreteStateId>("42").is_err());

    let id = ConcreteStateId::new("native:main-screen").unwrap();
    let encoded = serde_json::to_string(&id).unwrap();
    let decoded: ConcreteStateId = serde_json::from_str(&encoded).unwrap();
    assert_eq!(decoded, id);
    assert_eq!(decoded.as_str(), "native:main-screen");
}

#[test]
fn nested_id_deserialization_does_not_bypass_validation() {
    let mut encoded = serde_json::to_value(observation()).unwrap();
    encoded["state"]["widgets"][0]["id"] = json!(" ");
    assert!(serde_json::from_value::<Observation>(encoded).is_err());
}

#[test]
fn omitted_candidate_scope_defaults_to_unknown() {
    let mut observed = observation();
    observed.candidates.push(ActionCandidate {
        action: ActionId::new("activate").unwrap(),
        target: Some(WidgetId::new("button").unwrap()),
    });
    observed.candidate_scope = CandidateScope::Eligible;
    let mut encoded = serde_json::to_value(&observed).unwrap();
    assert_eq!(encoded["candidate_scope"], json!("eligible"));
    encoded.as_object_mut().unwrap().remove("candidate_scope");

    let decoded: Observation = serde_json::from_value(encoded).unwrap();
    decoded.validate().unwrap();
    assert_eq!(decoded.candidates, observed.candidates);
    assert_eq!(decoded.candidate_scope, CandidateScope::Unknown);

    observed.candidate_scope = CandidateScope::Generated;
    let generated = serde_json::to_value(&observed).unwrap();
    assert_eq!(generated["candidate_scope"], json!("generated"));
    let decoded: Observation = serde_json::from_value(generated).unwrap();
    assert_eq!(decoded.candidate_scope, CandidateScope::Generated);
}

#[test]
fn observed_properties_preserve_structured_values_and_explicit_null() {
    let mut observed = observation();
    observed
        .state
        .properties
        .insert("sut:mode".into(), json!("main"));
    observed
        .state
        .properties
        .insert("sut:empty".into(), json!(null));
    observed.state.widgets[1]
        .properties
        .insert("ui:enabled".into(), json!(true));
    observed.state.widgets[1].properties.insert(
        "ui:geometry".into(),
        json!({"position": [5, 10], "size": {"width": 100, "height": 30}}),
    );
    observed.validate().unwrap();

    let encoded = serde_json::to_vec(&observed).unwrap();
    let decoded: Observation = serde_json::from_slice(&encoded).unwrap();
    decoded.validate().unwrap();
    assert_eq!(decoded, observed);
    assert_eq!(
        decoded.state.properties.get("sut:empty"),
        Some(&json!(null))
    );
    assert!(!decoded.state.properties.contains_key("sut:unobserved"));
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

    let decoded: ConcreteState =
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
fn observation_candidates_bind_to_local_widgets_or_global_actions() {
    let mut observed = observation();
    observed.candidates = vec![
        ActionCandidate {
            action: ActionId::new("activate").unwrap(),
            target: Some(WidgetId::new("button").unwrap()),
        },
        ActionCandidate {
            action: ActionId::new("restart").unwrap(),
            target: None,
        },
    ];
    observed.validate().unwrap();

    observed.candidates[0].target =
        Some(WidgetId::new("another-observation-widget").unwrap());
    assert!(matches!(
        observed.validate(),
        Err(Error::UnknownReference(_))
    ));
}

#[test]
fn duplicate_candidates_do_not_inflate_a_single_observation() {
    let mut observed = observation();
    let candidate = ActionCandidate {
        action: ActionId::new("activate").unwrap(),
        target: Some(WidgetId::new("button").unwrap()),
    };
    observed.candidates = vec![candidate.clone(), candidate];
    assert!(matches!(observed.validate(), Err(Error::Conflict(_))));
}

#[test]
fn distinct_observations_preserve_their_own_concrete_data() {
    let first = observation();
    let mut second = first.clone();
    second.id = ObservationId::new("observation-2").unwrap();
    second.sequence = 1;
    second.state.id = ConcreteStateId::new("snapshot-2").unwrap();
    second.state.widgets[1]
        .properties
        .insert("ui:text".into(), json!("changed"));
    first.validate().unwrap();
    second.validate().unwrap();

    assert_ne!(first.id, second.id);
    assert_ne!(first.state, second.state);
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
    reference.validate().unwrap();
    reference.width = Some(0);
    assert!(reference.validate().is_err());
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
fn check_results_retain_their_explicit_scope_and_uncertainty() {
    let result = CheckResult {
        run: RunId::new("run-1").unwrap(),
        target: CheckTarget::Run {},
        rule: CheckRule {
            id: "example:eventually-completes".into(),
            version: None,
            parameters: None,
        },
        expected: ExpectedValue::Unspecified,
        actual: None,
        name: "operation eventually completes".into(),
        status: CheckStatus::Pending,
        details: PropertyMap::new(),
    };
    result.validate().unwrap();
    let encoded = serde_json::to_value(&result).unwrap();
    assert_eq!(encoded["status"], json!("pending"));
    let decoded: CheckResult = serde_json::from_value(encoded).unwrap();
    assert_eq!(decoded, result);
    assert_ne!(decoded.status, CheckStatus::Passed);
}
