use serde::{Deserialize, Serialize};

use crate::{
    ActionDefinition, CheckResult, Error, EventId, Execution, ExecutionId,
    ModelId, Observation, ObservationId, Result, Run, RunId,
};

pub const SCHEMA_VERSION: u32 = 2;

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Manifest {
    pub schema_version: u32,
    pub model: ModelId,
}

impl Manifest {
    pub fn validate(&self) -> Result<()> {
        validate_version(self.schema_version)?;
        Ok(())
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct EvidenceEvent {
    pub schema_version: u32,
    pub id: EventId,
    pub event: Event,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "kind", content = "data", rename_all = "snake_case")]
pub enum Event {
    RunStarted(Run),
    ActionDefined(ActionDefinition),
    Observed(Observation),
    ActionSelected(Execution),
    ActionSubmitted {
        execution: ExecutionId,
        accepted: bool,
        error: Option<String>,
    },
    ActionResult {
        execution: ExecutionId,
        observation: ObservationId,
    },
    Checked(CheckResult),
    RunFinished {
        run: RunId,
        reason: String,
    },
}

pub(crate) fn validate_version(version: u32) -> Result<()> {
    if version != SCHEMA_VERSION {
        return Err(Error::InvalidRecord(format!(
            "unsupported schema version {version}; expected {SCHEMA_VERSION}"
        )));
    }
    Ok(())
}
