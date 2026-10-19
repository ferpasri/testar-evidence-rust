use serde::{Deserialize, Serialize};

use super::properties::validate_property_name;
use super::{ExecutionId, ObservationId, WidgetId};
use crate::Result;

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum CheckTarget {
    Run {},
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
    Execution {
        execution: ExecutionId,
    },
    Sequence {
        from_observation: ObservationId,
        to_observation: ObservationId,
    },
}

impl CheckTarget {
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
