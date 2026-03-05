use {
    serde::{Deserialize, Deserializer, Serializer},
    serde_with::{DeserializeAs, SerializeAs},
};

/// Serialize and deserialize [`alloy::primitives::U256`] as a decimal string.
#[derive(Debug)]
pub struct U256;

impl<'de> DeserializeAs<'de, alloy::primitives::U256> for U256 {
    fn deserialize_as<D: Deserializer<'de>>(
        deserializer: D,
    ) -> Result<alloy::primitives::U256, D::Error> {
        alloy::primitives::U256::deserialize(deserializer)
    }
}

impl SerializeAs<alloy::primitives::U256> for U256 {
    fn serialize_as<S: Serializer>(
        value: &alloy::primitives::U256,
        serializer: S,
    ) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&value.to_string())
    }
}
