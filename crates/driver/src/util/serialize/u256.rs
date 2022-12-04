//! Serialize and deserialize [`U256`] as a decimal string.

use {
    ethereum_types,
    serde::{de, Deserializer, Serializer},
    serde_with::{DeserializeAs, SerializeAs},
};

#[derive(Debug)]
pub struct U256;

impl<'de> DeserializeAs<'de, ethereum_types::U256> for U256 {
    fn deserialize_as<D: Deserializer<'de>>(
        deserializer: D,
    ) -> Result<ethereum_types::U256, D::Error> {
        struct Visitor;

        impl<'de> de::Visitor<'de> for Visitor {
            type Value = ethereum_types::U256;

            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                write!(formatter, "a 256-bit decimal string")
            }

            fn visit_str<E>(self, s: &str) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                ethereum_types::U256::from_dec_str(s).map_err(|err| {
                    de::Error::custom(format!("failed to decode {s:?} as a 256-bit number: {err}"))
                })
            }
        }

        deserializer.deserialize_str(Visitor)
    }
}

impl SerializeAs<ethereum_types::U256> for U256 {
    fn serialize_as<S: Serializer>(
        source: &ethereum_types::U256,
        serializer: S,
    ) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&source.to_string())
    }
}
