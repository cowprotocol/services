use serde::{de, ser, Deserialize, Deserializer, Serialize, Serializer};

/// A type that never deserializes or serializes.
///
/// This can be used in situations where a generic type that implements `serde`
/// traits is required, but you don't want it to actually represent any data.
pub struct Never;

impl<'de> Deserialize<'de> for Never {
    fn deserialize<D>(_: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        Err(de::Error::custom("neva eva eva"))
    }
}

impl Serialize for Never {
    fn serialize<S>(&self, _: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        Err(ser::Error::custom("neva eva eva"))
    }
}
