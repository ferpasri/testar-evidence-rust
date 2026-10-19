use std::collections::BTreeMap;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::archive::{relative_path, unpack};
use crate::{Release, Result};

#[derive(Clone, Debug)]
pub struct PreparedSpecs {
    pub release: Release,
    pub root: PathBuf,
}

#[derive(Deserialize, Serialize)]
struct Snapshot {
    release: Release,
    directory: String,
    archive_sha256: String,
    files: BTreeMap<String, String>,
}

fn base(repository: &Path) -> PathBuf {
    repository.join("target/conformance")
}

fn digest(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

pub fn case_directories(root: &Path) -> Result<Vec<PathBuf>> {
    let mut cases = Vec::new();
    for entry in fs::read_dir(root.join("conformance/cases"))? {
        let entry = entry?;
        if !entry.file_type()?.is_dir() {
            continue;
        }
        let directory = entry.path();
        for required in ["manifest.json", "evidence.jsonl", "expected.json"] {
            if !directory.join(required).is_file() {
                return Err(format!(
                    "Incomplete conformance case {}: missing {required}",
                    directory.display()
                )
                .into());
            }
        }
        cases.push(directory);
    }
    cases.sort();
    if cases.is_empty() {
        return Err("Release contains no conformance cases".into());
    }
    Ok(cases)
}

pub fn load_prepared(repository: &Path) -> Result<PreparedSpecs> {
    let base = base(repository);
    let snapshot: Snapshot =
        serde_json::from_slice(&fs::read(base.join("current.json"))?)?;
    let directory = relative_path(&snapshot.directory)?;
    if directory.components().count() != 1 || snapshot.files.is_empty() {
        return Err("Invalid prepared specs metadata".into());
    }
    let root = base.join(directory).canonicalize()?;
    if !root.starts_with(base.canonicalize()?) {
        return Err("Prepared specs directory escapes the cache".into());
    }
    for (relative, expected) in &snapshot.files {
        let path = root.join(relative_path(relative)?);
        let resolved = path.canonicalize()?;
        if !resolved.starts_with(&root) {
            return Err("Prepared specs file escapes the cache".into());
        }
        let content = fs::read(&resolved)?;
        if digest(&content) != *expected {
            return Err(format!(
                "Prepared specs file changed: {}",
                path.display()
            )
            .into());
        }
    }
    case_directories(&root)?;
    Ok(PreparedSpecs {
        release: snapshot.release,
        root,
    })
}

pub(crate) fn install(
    repository: &Path,
    release: Release,
    content: &[u8],
) -> Result<PreparedSpecs> {
    let files = unpack(content)?;
    let archive_sha256 = digest(content);
    let cache = base(repository);
    if let Ok(previous) = load_prepared(repository) {
        let snapshot: Snapshot =
            serde_json::from_slice(&fs::read(cache.join("current.json"))?)?;
        if snapshot.release == release
            && snapshot.archive_sha256 == archive_sha256
        {
            return Ok(previous);
        }
    }
    fs::create_dir_all(&cache)?;
    let temporary = tempfile::Builder::new()
        .prefix("specs-")
        .tempdir_in(&cache)?;
    let mut hashes = BTreeMap::new();
    for (relative, bytes) in files {
        let destination = temporary.path().join(relative_path(&relative)?);
        fs::create_dir_all(destination.parent().unwrap())?;
        fs::write(destination, &bytes)?;
        hashes.insert(relative, digest(&bytes));
    }
    case_directories(temporary.path())?;
    let root = temporary.keep();
    let directory = root.file_name().unwrap().to_str().unwrap().to_owned();
    let snapshot = Snapshot {
        release: release.clone(),
        directory,
        archive_sha256,
        files: hashes,
    };
    let mut marker = tempfile::NamedTempFile::new_in(&cache)?;
    serde_json::to_writer_pretty(&mut marker, &snapshot)?;
    marker.write_all(b"\n")?;
    marker.as_file().sync_all()?;
    marker.persist(cache.join("current.json"))?;
    Ok(PreparedSpecs { release, root })
}
