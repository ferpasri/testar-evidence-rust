use serde::{Deserialize, Deserializer, Serialize, Serializer};
use serde_json::Value;

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub enum ExpectedValue {
    #[default]
    Unspecified,
    Value(Value),
}

impl ExpectedValue {
    pub fn is_unspecified(&self) -> bool {
        matches!(self, Self::Unspecified)
    }
}

impl Serialize for ExpectedValue {
    fn serialize<S: Serializer>(
        &self,
        serializer: S,
    ) -> Result<S::Ok, S::Error> {
        match self {
            Self::Unspecified => serializer.serialize_unit(),
            Self::Value(value) => value.serialize(serializer),
        }
    }
}

impl<'de> Deserialize<'de> for ExpectedValue {
    fn deserialize<D: Deserializer<'de>>(
        deserializer: D,
    ) -> Result<Self, D::Error> {
        Value::deserialize(deserializer).map(Self::Value)
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum CheckReading {
    Value { value: Value },
    Absent {},
    Unknown {},
}

pub(super) fn deserialize_present<'de, D, T>(
    deserializer: D,
) -> Result<Option<T>, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de>,
{
    T::deserialize(deserializer).map(Some)
}
