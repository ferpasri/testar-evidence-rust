use serde::{Deserialize, Serialize};

use super::properties::validate_text;
use crate::error::{Error, Result};

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ArtifactReference {
    pub id: String,
    pub path: String,
    pub media_type: String,
    pub sha256: String,
    pub width: Option<u32>,
    pub height: Option<u32>,
}

impl ArtifactReference {
    pub fn validate(&self) -> Result<()> {
        validate_text(&self.id, "artifact ID")?;
        validate_text(&self.media_type, "artifact media type")?;
        validate_artifact_path(&self.path)?;
        if self.sha256.len() != 64
            || !self.sha256.bytes().all(|byte| byte.is_ascii_hexdigit())
        {
            return Err(Error::InvalidRecord(
                "artifact SHA-256 must contain 64 hexadecimal characters"
                    .into(),
            ));
        }
        if self.width == Some(0) || self.height == Some(0) {
            return Err(Error::InvalidRecord(
                "artifact dimensions must be positive when supplied".into(),
            ));
        }
        Ok(())
    }
}

fn validate_artifact_path(path: &str) -> Result<()> {
    validate_text(path, "artifact path")?;
    if path.contains(['\\', ':']) || path.starts_with('/') {
        return Err(Error::InvalidRecord(
            "artifact path must be a portable relative path using forward slashes".into(),
        ));
    }

    for component in path.split('/') {
        if component.is_empty()
            || component == "."
            || component == ".."
            || component.ends_with(['.', ' '])
            || component.contains(['<', '>', '"', '|', '?', '*'])
            || is_device_name(component)
        {
            return Err(Error::InvalidRecord(format!(
                "unsafe artifact path component {component:?}"
            )));
        }
    }
    Ok(())
}

fn is_device_name(component: &str) -> bool {
    let stem = component
        .split('.')
        .next()
        .unwrap_or("")
        .to_ascii_uppercase();
    if matches!(stem.as_str(), "CON" | "PRN" | "AUX" | "NUL") {
        return true;
    }
    let bytes = stem.as_bytes();
    bytes.len() == 4
        && (&bytes[..3] == b"COM" || &bytes[..3] == b"LPT")
        && matches!(bytes[3], b'1'..=b'9')
}
