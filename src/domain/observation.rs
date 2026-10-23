use super::properties::{validate_properties, validate_text};
use super::{
    ArtifactReference, AvailableAction, ObservationId, PropertyMap,
    RecordedState, TestRunId, TestSequenceId,
};
use crate::{Error, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct TestRun {
    pub id: TestRunId,
    pub producer: String,
    pub source_run: String,
    pub properties: PropertyMap,
}

impl TestRun {
    pub fn validate(&self) -> Result<()> {
        validate_text(&self.producer, "producer")?;
        validate_text(&self.source_run, "source_run")?;
        validate_properties(&self.properties)
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct TestSequence {
    pub id: TestSequenceId,
    pub test_run: TestRunId,
    pub sequence_index: u64,
    pub source_sequence: String,
    pub properties: PropertyMap,
}

impl TestSequence {
    pub fn validate(&self) -> Result<()> {
        if self.sequence_index == 0 {
            return Err(Error::InvalidRecord(
                "sequence_index must be positive".into(),
            ));
        }
        validate_text(&self.source_sequence, "source_sequence")?;
        validate_properties(&self.properties)
    }
}

#[derive(
    Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize,
)]
#[serde(rename_all = "snake_case")]
pub enum AvailableActionScope {
    #[default]
    Unknown,
    Generated,
    Eligible,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Observation {
    pub id: ObservationId,
    pub test_sequence: TestSequenceId,
    pub observation_index: u64,
    pub timestamp_micros: u64,
    pub recorded_state: RecordedState,
    pub available_actions: Vec<AvailableAction>,
    pub available_action_scope: AvailableActionScope,
    pub artifacts: Vec<ArtifactReference>,
}

impl Observation {
    pub fn validate(&self) -> Result<()> {
        self.recorded_state.validate()?;
        let widgets: HashSet<_> = self
            .recorded_state
            .widgets
            .iter()
            .map(|widget| &widget.id)
            .collect();
        let mut actions = HashSet::new();
        for action in &self.available_actions {
            if action
                .target_widget
                .as_ref()
                .is_some_and(|target| !widgets.contains(target))
            {
                return Err(Error::UnknownReference(
                    "available action target widget".into(),
                ));
            }
            if !actions
                .insert((&action.action_description, &action.target_widget))
            {
                return Err(Error::Conflict(
                    "duplicate available action".into(),
                ));
            }
        }
        let mut artifacts = HashSet::new();
        for artifact in &self.artifacts {
            artifact.validate()?;
            if !artifacts.insert(&artifact.id) {
                return Err(Error::Conflict(
                    "duplicate artifact reference".into(),
                ));
            }
        }
        Ok(())
    }
}
