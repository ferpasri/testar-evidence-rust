use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use super::properties::validate_properties;
use super::{ConcreteStateId, PropertyMap, WidgetId};
use crate::error::{Error, Result};

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CaptureScope {
    Complete,
    Filtered,
    Unavailable,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Widget {
    pub id: WidgetId,
    pub parent: Option<WidgetId>,
    pub properties: PropertyMap,
}

impl Widget {
    pub fn validate(&self) -> Result<()> {
        validate_properties(&self.properties)?;
        if self.parent.as_ref() == Some(&self.id) {
            return Err(Error::InvalidRecord(format!(
                "widget {} cannot be its own parent",
                self.id
            )));
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ConcreteState {
    pub id: ConcreteStateId,
    pub properties: PropertyMap,
    pub widgets: Vec<Widget>,
    pub capture: CaptureScope,
}

impl ConcreteState {
    pub fn validate(&self) -> Result<()> {
        validate_properties(&self.properties)?;
        if self.capture == CaptureScope::Unavailable && !self.widgets.is_empty()
        {
            return Err(Error::InvalidRecord(
                "unavailable widget capture cannot contain widgets".into(),
            ));
        }

        let mut widget_indices = HashMap::new();
        for (index, widget) in self.widgets.iter().enumerate() {
            widget.validate()?;
            if widget_indices.insert(&widget.id, index).is_some() {
                return Err(Error::Conflict(format!(
                    "duplicate widget {} in concrete state {}",
                    widget.id, self.id
                )));
            }
        }

        for widget in &self.widgets {
            if let Some(parent) = &widget.parent {
                if !widget_indices.contains_key(parent) {
                    return Err(Error::UnknownReference(format!(
                        "parent widget {parent} of widget {}",
                        widget.id
                    )));
                }
            }
        }

        self.validate_hierarchy(&widget_indices)
    }

    fn validate_hierarchy(
        &self,
        widget_indices: &HashMap<&WidgetId, usize>,
    ) -> Result<()> {
        let mut visitation = vec![0; self.widgets.len()];
        for start in 0..self.widgets.len() {
            if visitation[start] == 2 {
                continue;
            }

            let mut ancestry = Vec::new();
            let mut current = Some(start);
            while let Some(index) = current {
                if visitation[index] == 2 {
                    break;
                }
                if visitation[index] == 1 {
                    return Err(Error::InvalidRecord(format!(
                        "widget hierarchy contains a cycle at {}",
                        self.widgets[index].id
                    )));
                }

                visitation[index] = 1;
                ancestry.push(index);
                current = self.widgets[index]
                    .parent
                    .as_ref()
                    .map(|parent| widget_indices[parent]);
            }

            for index in ancestry {
                visitation[index] = 2;
            }
        }
        Ok(())
    }
}
