use super::properties::{validate_properties, validate_text};
use super::{
    ActionDescriptionId, ActionExecutionId, ObservationId, PropertyMap,
    TestSequenceId, WidgetId,
};
use crate::{Error, Result};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ActionDescription {
    pub id: ActionDescriptionId,
    pub action_type: String,
    pub properties: PropertyMap,
}

impl ActionDescription {
    pub fn validate(&self) -> Result<()> {
        validate_text(&self.action_type, "action_type")?;
        validate_properties(&self.properties)
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct AvailableAction {
    pub action_description: ActionDescriptionId,
    #[serde(deserialize_with = "super::oracle_values::deserialize_nullable")]
    pub target_widget: Option<WidgetId>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ActionExecution {
    pub id: ActionExecutionId,
    pub test_sequence: TestSequenceId,
    pub source_observation: ObservationId,
    pub action_description: ActionDescriptionId,
    #[serde(deserialize_with = "super::oracle_values::deserialize_nullable")]
    pub target_widget: Option<WidgetId>,
    pub parameters: PropertyMap,
}

impl ActionExecution {
    pub fn validate(&self) -> Result<()> {
        validate_properties(&self.parameters)
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ActionOutcome {
    Returned,
    Error,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ActionExecutionResult {
    pub action_execution: ActionExecutionId,
    pub outcome: ActionOutcome,
    #[serde(deserialize_with = "super::oracle_values::deserialize_nullable")]
    pub error: Option<String>,
}

impl ActionExecutionResult {
    pub fn validate(&self) -> Result<()> {
        if (self.outcome == ActionOutcome::Returned) != self.error.is_none() {
            return Err(Error::InvalidRecord("returned requires a null error; error outcome requires its message".into()));
        }
        Ok(())
    }
}
