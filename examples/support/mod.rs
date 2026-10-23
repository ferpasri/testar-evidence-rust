use std::fs;
use std::io;
use std::path::{Path, PathBuf};

pub fn create_trace_directory(
    parent: &Path,
    sut: &str,
    test_objective: &str,
) -> io::Result<PathBuf> {
    let timestamp = chrono::Local::now()
        .format("%Y-%m-%d_%H-%M-%S-%3f")
        .to_string();
    create_at(parent, sut, test_objective, &timestamp)
}

pub fn create_at(
    parent: &Path,
    sut: &str,
    test_objective: &str,
    timestamp: &str,
) -> io::Result<PathBuf> {
    fs::create_dir_all(parent)?;
    let name = format!(
        "{}_{}_trace-{timestamp}",
        label(sut, "sut"),
        label(test_objective, "objective")
    );
    let mut suffix = 1_u64;
    loop {
        let candidate = if suffix == 1 {
            parent.join(&name)
        } else {
            parent.join(format!("{name}-{suffix}"))
        };
        match fs::create_dir(&candidate) {
            Ok(()) => return Ok(candidate),
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {
                suffix += 1;
            }
            Err(error) => return Err(error),
        }
    }
}

fn label(value: &str, fallback: &str) -> String {
    let mut cleaned = String::new();
    for character in value.chars() {
        if character.is_ascii_alphanumeric() || "-_".contains(character) {
            cleaned.push(character);
        } else if !cleaned.ends_with('-') {
            cleaned.push('-');
        }
    }
    let cleaned = cleaned.trim_matches(['-', '_']);
    let cleaned = &cleaned[..cleaned.len().min(32)];
    let cleaned = cleaned.trim_end_matches(['-', '_']);
    if cleaned.is_empty() {
        fallback.to_owned()
    } else {
        cleaned.to_owned()
    }
}
