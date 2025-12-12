use {
    alloy::primitives::U256,
    serde::{Deserialize, Deserializer, Serializer},
    serde_with::{DeserializeAs, SerializeAs},
};

/// (De)serialization structure able to deserialize decimal and hexadecimal
/// numbers, serializes as decimal.
pub struct HexOrDecimalU256;

impl<'de> DeserializeAs<'de, U256> for HexOrDecimalU256 {
    fn deserialize_as<D>(deserializer: D) -> Result<U256, D::Error>
    where
        D: Deserializer<'de>,
    {
        U256::deserialize(deserializer)
    }
}

impl SerializeAs<U256> for HexOrDecimalU256 {
    fn serialize_as<S>(source: &U256, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        // alloy::primitives::U256 serializes as hex, this gives us decimals instead
        serializer.serialize_str(&source.to_string())
    }
}
