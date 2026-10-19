use serde::{Deserialize, Serialize};

use super::check_values::deserialize_present;
use super::properties::{validate_properties, validate_text};
use super::{CheckReading, CheckTarget, ExpectedValue, PropertyMap, RunId};
use crate::Result;

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CheckStatus {
    Passed,
    Failed,
    Pending,
    Unknown,
    Error,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CheckRule {
    pub id: String,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "deserialize_present"
    )]
    pub version: Option<String>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "deserialize_present"
    )]
    pub parameters: Option<PropertyMap>,
}

impl CheckRule {
    pub fn validate(&self) -> Result<()> {
        validate_text(&self.id, "rule id")?;
        if let Some(version) = &self.version {
            validate_text(version, "rule version")?;
        }
        if let Some(parameters) = &self.parameters {
            validate_properties(parameters)?;
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CheckResult {
    pub run: RunId,
    pub name: String,
    pub rule: CheckRule,
    pub target: CheckTarget,
    pub status: CheckStatus,
    #[serde(default, skip_serializing_if = "ExpectedValue::is_unspecified")]
    pub expected: ExpectedValue,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "deserialize_present"
    )]
    pub actual: Option<CheckReading>,
    #[serde(default)]
    pub details: PropertyMap,
}

impl CheckResult {
    pub fn validate(&self) -> Result<()> {
        validate_text(&self.name, "check name")?;
        self.rule.validate()?;
        self.target.validate()?;
        validate_properties(&self.details)
    }
}
