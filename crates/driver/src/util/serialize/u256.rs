use {
    crate::domain::eth,
    serde::{Deserializer, Serializer, de},
    serde_with::{DeserializeAs, SerializeAs},
};

// NOTE(jmg-duarte): not sure if we still need this module

/// Serialize and deserialize [`eth::U256`] as a decimal string.
#[derive(Debug)]
pub struct U256;

impl<'de> DeserializeAs<'de, eth::U256> for U256 {
    fn deserialize_as<D: Deserializer<'de>>(deserializer: D) -> Result<eth::U256, D::Error> {
        struct Visitor;

        impl de::Visitor<'_> for Visitor {
            type Value = eth::U256;

            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                write!(formatter, "a 256-bit decimal string")
            }

            fn visit_str<E>(self, s: &str) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                eth::U256::from_str_radix(s, 10).map_err(|err| {
                    de::Error::custom(format!("failed to decode {s:?} as a 256-bit number: {err}"))
                })
            }
        }

        deserializer.deserialize_str(Visitor)
    }
}

impl SerializeAs<eth::U256> for U256 {
    fn serialize_as<S: Serializer>(source: &eth::U256, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&source.to_string())
    }
}
