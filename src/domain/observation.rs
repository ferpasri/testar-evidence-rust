use std::collections::HashSet;

use serde::{Deserialize, Serialize};

use super::properties::{validate_properties, validate_text};
use super::{
    ActionCandidate, ArtifactReference, ConcreteState, ObservationId,
    PropertyMap, RunId,
};
use crate::error::{Error, Result};

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Run {
    pub id: RunId,
    pub producer: String,
    pub source_run: String,
    pub properties: PropertyMap,
}

impl Run {
    pub fn validate(&self) -> Result<()> {
        validate_text(&self.producer, "run producer")?;
        validate_text(&self.source_run, "producer run identifier")?;
        validate_properties(&self.properties)
    }
}

#[derive(
    Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize,
)]
#[serde(rename_all = "snake_case")]
pub enum CandidateScope {
    #[default]
    Unknown,
    Generated,
    Eligible,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Observation {
    pub id: ObservationId,
    pub run: RunId,
    pub sequence: u64,
    pub timestamp_micros: u64,
    pub state: ConcreteState,
    pub candidates: Vec<ActionCandidate>,
    #[serde(default)]
    pub candidate_scope: CandidateScope,
    pub artifacts: Vec<ArtifactReference>,
}

impl Observation {
    pub fn validate(&self) -> Result<()> {
        self.state.validate()?;
        let widgets: HashSet<_> =
            self.state.widgets.iter().map(|widget| &widget.id).collect();
        let mut candidates = HashSet::new();
        for candidate in &self.candidates {
            candidate.validate()?;
            if let Some(target) = &candidate.target {
                if !widgets.contains(target) {
                    return Err(Error::UnknownReference(format!(
                        "candidate target widget {target} in observation {}",
                        self.id
                    )));
                }
            }
            if !candidates.insert((&candidate.action, &candidate.target)) {
                return Err(Error::Conflict(format!(
                    "duplicate action candidate {} in observation {}",
                    candidate.action, self.id
                )));
            }
        }

        let mut artifacts = HashSet::new();
        for artifact in &self.artifacts {
            artifact.validate()?;
            if !artifacts.insert(&artifact.id) {
                return Err(Error::Conflict(format!(
                    "duplicate artifact {} in observation {}",
                    artifact.id, self.id
                )));
            }
        }
        Ok(())
    }
}
