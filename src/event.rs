use crate::{
    ActionDescription, ActionExecution, ActionExecutionId,
    ActionExecutionResult, Error, EventId, EvidenceId, Observation,
    ObservationId, OracleResult, Result, TestRun, TestRunId, TestSequence,
    TestSequenceId,
};
use serde::{Deserialize, Serialize};

pub const SCHEMA_VERSION: u32 = 3;

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Manifest {
    pub schema_version: u32,
    pub evidence_id: EvidenceId,
}

impl Manifest {
    pub fn validate(&self) -> Result<()> {
        validate_version(self.schema_version)
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct EvidenceEvent {
    pub schema_version: u32,
    pub id: EventId,
    pub event: Event,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(
    tag = "event_type",
    content = "data",
    rename_all = "snake_case",
    deny_unknown_fields
)]
pub enum Event {
    TestRunStarted(TestRun),
    TestSequenceStarted(TestSequence),
    ActionDescribed(ActionDescription),
    ObservationRecorded(Observation),
    ActionSelected(ActionExecution),
    ActionExecutionReported(ActionExecutionResult),
    TransitionRecorded {
        action_execution: ActionExecutionId,
        resulting_observation: ObservationId,
    },
    OracleResultRecorded(OracleResult),
    TestSequenceFinished {
        test_sequence: TestSequenceId,
        reason: String,
    },
    TestRunFinished {
        test_run: TestRunId,
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
