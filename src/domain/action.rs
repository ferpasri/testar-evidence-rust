use serde::{Deserialize, Serialize};

use super::properties::{validate_properties, validate_text};
use super::{
    ActionId, ExecutionId, ObservationId, PropertyMap, RunId, WidgetId,
};
use crate::error::Result;

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ActionDefinition {
    pub id: ActionId,
    pub kind: String,
    pub properties: PropertyMap,
}

impl ActionDefinition {
    pub fn validate(&self) -> Result<()> {
        validate_text(&self.kind, "action kind")?;
        validate_properties(&self.properties)
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ActionCandidate {
    pub action: ActionId,
    pub target: Option<WidgetId>,
}

impl ActionCandidate {
    pub fn validate(&self) -> Result<()> {
        Ok(())
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Execution {
    pub id: ExecutionId,
    pub run: RunId,
    pub source: ObservationId,
    pub action: ActionId,
    pub target: Option<WidgetId>,
    pub parameters: PropertyMap,
}

impl Execution {
    pub fn validate(&self) -> Result<()> {
        validate_properties(&self.parameters)
    }
}
