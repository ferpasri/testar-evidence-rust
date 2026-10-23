use super::oracle_values::deserialize_present;
use super::properties::{validate_properties, validate_text};
use super::{
    ExpectedValue, ObservedValue, OracleTarget, PropertyMap, TestRunId,
    TestSequenceId,
};
use crate::{Error, Result};
use serde::{Deserialize, Serialize};
use serde_json::Number;

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum OracleStatus {
    Passed,
    Failed,
    Pending,
    Unknown,
    Error,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Oracle {
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

impl Oracle {
    pub fn validate(&self) -> Result<()> {
        validate_text(&self.id, "oracle.id")?;
        if let Some(version) = &self.version {
            validate_text(version, "oracle.version")?;
        }
        if let Some(parameters) = &self.parameters {
            validate_properties(parameters)?;
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct OracleSeverity {
    pub label: String,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "deserialize_present"
    )]
    pub value: Option<Number>,
}

impl OracleSeverity {
    pub fn validate(&self) -> Result<()> {
        validate_text(&self.label, "oracle_severity.label").map_err(|error| {
            Error::Validation {
                code: "invalid_oracle_severity",
                field: "oracle_severity.label",
                message: error.to_string(),
            }
        })
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct OracleResult {
    pub test_run: TestRunId,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "deserialize_present"
    )]
    pub test_sequence: Option<TestSequenceId>,
    pub name: String,
    pub oracle: Oracle,
    pub oracle_target: OracleTarget,
    pub oracle_status: OracleStatus,
    #[serde(default, skip_serializing_if = "ExpectedValue::is_unspecified")]
    pub expected_value: ExpectedValue,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "deserialize_present"
    )]
    pub observed_value: Option<ObservedValue>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "deserialize_present"
    )]
    pub oracle_severity: Option<OracleSeverity>,
    pub oracle_details: PropertyMap,
}

impl OracleResult {
    pub fn validate(&self) -> Result<()> {
        validate_text(&self.name, "oracle name")?;
        self.oracle.validate()?;
        self.oracle_target.validate()?;
        if let Some(severity) = &self.oracle_severity {
            severity.validate()?;
        }
        validate_properties(&self.oracle_details)
    }
}
