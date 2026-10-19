use std::collections::BTreeMap;

use crate::error::{Error, Result};

pub type PropertyMap = BTreeMap<String, serde_json::Value>;

pub(crate) fn validate_properties(properties: &PropertyMap) -> Result<()> {
    for key in properties.keys() {
        validate_property_name(key)?;
    }
    Ok(())
}

pub(crate) fn validate_property_name(name: &str) -> Result<()> {
    validate_text(name, "property name")
}

pub(crate) fn validate_text(value: &str, field: &str) -> Result<()> {
    if value.trim().is_empty() || value.chars().any(char::is_control) {
        return Err(Error::InvalidRecord(format!(
            "{field} must not be blank or contain control characters"
        )));
    }
    Ok(())
}
