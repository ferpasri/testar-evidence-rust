use std::collections::BTreeMap;
use std::io::{Cursor, Read};
use std::path::{Component, Path, PathBuf};

use zip::ZipArchive;

use crate::Result;

pub(crate) fn relative_path(name: &str) -> Result<PathBuf> {
    let path = Path::new(name);
    if name.is_empty()
        || name.contains(['\\', ':'])
        || path
            .components()
            .any(|part| !matches!(part, Component::Normal(_)))
    {
        return Err(format!("Unsafe conformance path: {name}").into());
    }
    Ok(path.to_path_buf())
}

pub(crate) fn unpack(content: &[u8]) -> Result<BTreeMap<String, Vec<u8>>> {
    let mut archive = ZipArchive::new(Cursor::new(content))?;
    let mut files = BTreeMap::new();
    let mut total = 0;
    for index in 0..archive.len() {
        let file = archive.by_index(index)?;
        if file.is_dir() {
            continue;
        }
        let Some((_, relative)) = file.name().split_once('/') else {
            continue;
        };
        if !relative.starts_with("conformance/") {
            continue;
        }
        let relative = relative.to_owned();
        relative_path(&relative)?;
        if file
            .unix_mode()
            .is_some_and(|mode| mode & 0o170000 == 0o120000)
        {
            return Err(
                "Conformance archives cannot contain symbolic links".into()
            );
        }
        let mut bytes = Vec::new();
        file.take(32 * 1024 * 1024 + 1).read_to_end(&mut bytes)?;
        total += bytes.len();
        if bytes.len() > 32 * 1024 * 1024 || total > 128 * 1024 * 1024 {
            return Err(
                "Conformance archive exceeds the extraction limit".into()
            );
        }
        if files.insert(relative.clone(), bytes).is_some() {
            return Err(
                format!("Duplicate conformance entry: {relative}").into()
            );
        }
    }
    if files.is_empty() {
        return Err("Release archive contains no conformance files".into());
    }
    Ok(files)
}
