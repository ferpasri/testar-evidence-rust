#[path = "../examples/support/mod.rs"]
mod paths;

#[test]
fn labels_timestamp_and_collision_preserve_existing_evidence() {
    let temporary = tempfile::tempdir().unwrap();
    let first = paths::create_at(
        temporary.path(),
        "parabank",
        "denied-loan",
        "2026-10-22_15-30-12-123",
    )
    .unwrap();
    std::fs::write(first.join("keep.txt"), b"existing evidence").unwrap();
    let second = paths::create_at(
        temporary.path(),
        "parabank",
        "denied-loan",
        "2026-10-22_15-30-12-123",
    )
    .unwrap();
    assert_eq!(
        first.file_name().unwrap(),
        "parabank_denied-loan_trace-2026-10-22_15-30-12-123"
    );
    assert_eq!(
        second.file_name().unwrap(),
        "parabank_denied-loan_trace-2026-10-22_15-30-12-123-2"
    );
    assert_eq!(
        std::fs::read(first.join("keep.txt")).unwrap(),
        b"existing evidence"
    );
    assert!(!second.join("keep.txt").exists());
}

#[test]
fn labels_are_safe_and_bounded_with_fallbacks() {
    let temporary = tempfile::tempdir().unwrap();
    let path = paths::create_trace_directory(
        temporary.path(),
        "../../App: one?",
        "C:\\work\\<test>",
    )
    .unwrap();
    assert_eq!(path.parent().unwrap(), temporary.path());
    let name = path.file_name().unwrap().to_str().unwrap();
    assert!(
        !name
            .chars()
            .any(|character| "<>:\"/\\|?*".contains(character))
    );
    let fallback =
        paths::create_trace_directory(temporary.path(), "...", "???").unwrap();
    assert!(
        fallback
            .file_name()
            .unwrap()
            .to_str()
            .unwrap()
            .starts_with("sut_objective")
    );
    let bounded = paths::create_trace_directory(
        temporary.path(),
        &"a".repeat(1000),
        &"b".repeat(1000),
    )
    .unwrap();
    assert!(bounded.file_name().unwrap().len() < 120);
}
