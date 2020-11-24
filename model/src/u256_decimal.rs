use primitive_types::U256;
use serde::{de, Deserializer, Serializer};
use std::fmt;

pub fn serialize<S>(value: &U256, serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    serializer.serialize_str(&value.to_string())
}

pub fn deserialize<'de, D>(deserializer: D) -> Result<U256, D::Error>
where
    D: Deserializer<'de>,
{
    struct Visitor {}
    impl<'de> de::Visitor<'de> for Visitor {
        type Value = U256;

        fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
            write!(formatter, "a u256 encoded as a decimal encoded string")
        }

        fn visit_str<E>(self, s: &str) -> Result<Self::Value, E>
        where
            E: de::Error,
        {
            U256::from_dec_str(s).map_err(|err| {
                de::Error::custom(format!("failed to decode {:?} as decimal u256: {}", s, err))
            })
        }
    }

    deserializer.deserialize_str(Visitor {})
}
