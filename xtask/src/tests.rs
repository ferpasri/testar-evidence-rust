use std::io::{Cursor, Write};

use tempfile::tempdir;
use zip::ZipWriter;
use zip::write::SimpleFileOptions;

use crate::archive::unpack;
use crate::cache::install;
use crate::release::newest;
use crate::{Release, case_directories, load_prepared};

fn release(id: u64, date: &str, prerelease: bool) -> Release {
    Release {
        id,
        tag_name: format!("v{id}"),
        published_at: Some(date.into()),
        draft: false,
        prerelease,
        zipball_url: "https://example.org/specs.zip".into(),
    }
}

fn archive(files: &[(&str, &[u8])]) -> Vec<u8> {
    let mut writer = ZipWriter::new(Cursor::new(Vec::new()));
    for (name, content) in files {
        writer
            .start_file(format!("specs/{name}"), SimpleFileOptions::default())
            .unwrap();
        writer.write_all(content).unwrap();
    }
    writer.finish().unwrap().into_inner()
}

fn valid_archive() -> Vec<u8> {
    archive(&[
        ("conformance/cases/example/manifest.json", b"{}"),
        ("conformance/cases/example/evidence.jsonl", b""),
        ("conformance/cases/example/expected.json", b"{}"),
    ])
}

#[test]
fn newest_publication_includes_prereleases_and_ignores_input_order() {
    let selected = newest(vec![
        release(2, "2026-10-02T00:00:00Z", true),
        release(1, "2026-10-01T00:00:00Z", false),
        release(3, "2026-09-01T00:00:00Z", false),
    ])
    .unwrap();
    assert_eq!(selected.id, 2);
    assert!(selected.prerelease);
}

#[test]
fn drafts_and_unpublished_entries_are_excluded() {
    let mut draft = release(3, "2026-10-03T00:00:00Z", true);
    draft.draft = true;
    let mut unpublished = release(4, "2026-10-04T00:00:00Z", false);
    unpublished.published_at = None;
    assert_eq!(
        newest(vec![
            draft.clone(),
            unpublished,
            release(1, "2026-10-01T00:00:00Z", false)
        ])
        .unwrap()
        .id,
        1
    );
    assert!(newest(vec![draft]).is_err());
    assert!(newest(vec![]).is_err());
}

#[test]
fn identical_publication_dates_have_a_deterministic_tiebreaker() {
    assert_eq!(
        newest(vec![
            release(1, "2026-10-01T00:00:00Z", false),
            release(2, "2026-10-01T00:00:00Z", true),
        ])
        .unwrap()
        .id,
        2
    );
}

#[test]
fn install_reopens_offline_and_reuses_identical_verified_data() {
    let repository = tempdir().unwrap();
    let selected = release(1, "2026-10-01T00:00:00Z", true);
    let content = valid_archive();
    let prepared =
        install(repository.path(), selected.clone(), &content).unwrap();
    let reopened = load_prepared(repository.path()).unwrap();
    assert_eq!(reopened.release, selected);
    assert_eq!(case_directories(&reopened.root).unwrap().len(), 1);
    assert_eq!(
        std::fs::read(
            reopened
                .root
                .join("conformance/cases/example/manifest.json")
        )
        .unwrap(),
        b"{}"
    );
    let repeated = install(repository.path(), selected, &content).unwrap();
    assert_eq!(
        repeated.root.canonicalize().unwrap(),
        prepared.root.canonicalize().unwrap()
    );
}

#[test]
fn corrupted_files_are_detected_and_repaired_by_new_installation() {
    let repository = tempdir().unwrap();
    let selected = release(1, "2026-10-01T00:00:00Z", false);
    let content = valid_archive();
    let prepared =
        install(repository.path(), selected.clone(), &content).unwrap();
    std::fs::write(
        prepared
            .root
            .join("conformance/cases/example/manifest.json"),
        b"changed",
    )
    .unwrap();
    assert!(load_prepared(repository.path()).is_err());
    install(repository.path(), selected, &content).unwrap();
    assert!(load_prepared(repository.path()).is_ok());
}

#[test]
fn failed_installation_keeps_the_previous_release_selected() {
    let repository = tempdir().unwrap();
    install(
        repository.path(),
        release(1, "2026-10-01T00:00:00Z", false),
        &valid_archive(),
    )
    .unwrap();
    let incomplete = archive(&[("conformance/cases/new/manifest.json", b"{}")]);
    assert!(
        install(
            repository.path(),
            release(2, "2026-10-02T00:00:00Z", true),
            &incomplete
        )
        .is_err()
    );
    assert_eq!(load_prepared(repository.path()).unwrap().release.id, 1);
}

#[test]
fn malformed_archives_and_unsafe_paths_are_rejected() {
    assert!(unpack(b"not a zip").is_err());
    for name in [
        "conformance/../../escape.json",
        "conformance/C:/escape.json",
        "conformance/..\\escape.json",
    ] {
        assert!(unpack(&archive(&[(name, b"{}")])).is_err());
    }
    assert!(unpack(&archive(&[("README.md", b"no cases")])).is_err());
}

#[test]
fn discovery_includes_every_case_and_rejects_incomplete_cases() {
    let repository = tempdir().unwrap();
    let prepared = install(
        repository.path(),
        release(1, "2026-10-01T00:00:00Z", false),
        &valid_archive(),
    )
    .unwrap();
    let second = prepared.root.join("conformance/cases/second");
    std::fs::create_dir_all(&second).unwrap();
    assert!(case_directories(&prepared.root).is_err());
    for name in ["manifest.json", "evidence.jsonl", "expected.json"] {
        std::fs::write(second.join(name), b"{}").unwrap();
    }
    assert_eq!(case_directories(&prepared.root).unwrap().len(), 2);
}

#[test]
fn cache_metadata_cannot_select_a_directory_outside_the_cache() {
    let repository = tempdir().unwrap();
    install(
        repository.path(),
        release(1, "2026-10-01T00:00:00Z", false),
        &valid_archive(),
    )
    .unwrap();
    let marker = repository.path().join("target/conformance/current.json");
    let mut encoded: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&marker).unwrap()).unwrap();
    encoded["directory"] = serde_json::json!("../outside");
    std::fs::write(marker, serde_json::to_vec(&encoded).unwrap()).unwrap();
    assert!(load_prepared(repository.path()).is_err());
}
