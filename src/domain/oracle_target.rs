use serde::{Deserialize, Serialize};

use super::properties::validate_property_name;
use super::{ActionExecutionId, ObservationId, WidgetId};
use crate::Result;

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "target_type", rename_all = "snake_case", deny_unknown_fields)]
pub enum OracleTarget {
    TestRun {},
    TestSequence {},
    Observation {
        observation: ObservationId,
    },
    StateProperty {
        observation: ObservationId,
        property: String,
    },
    Widget {
        observation: ObservationId,
        widget: WidgetId,
    },
    WidgetProperty {
        observation: ObservationId,
        widget: WidgetId,
        property: String,
    },
    ActionExecution {
        action_execution: ActionExecutionId,
    },
    ObservationRange {
        first_observation: ObservationId,
        last_observation: ObservationId,
    },
}

impl OracleTarget {
    pub fn validate(&self) -> Result<()> {
        match self {
            Self::StateProperty { property, .. }
            | Self::WidgetProperty { property, .. } => {
                validate_property_name(property)
            }
            _ => Ok(()),
        }
    }
}
