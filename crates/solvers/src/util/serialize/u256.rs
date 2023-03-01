use {
    serde::{de, Deserialize, Deserializer, Serializer},
    serde_with::{DeserializeAs, SerializeAs},
    std::borrow::Cow,
};

/// Serialize and deserialize [`ethereum_types::U256`] as a decimal string.
#[derive(Debug)]
pub struct U256;

impl<'de> DeserializeAs<'de, ethereum_types::U256> for U256 {
    fn deserialize_as<D: Deserializer<'de>>(
        deserializer: D,
    ) -> Result<ethereum_types::U256, D::Error> {
        let s = Cow::<str>::deserialize(deserializer)?;
        ethereum_types::U256::from_dec_str(&s)
            .map_err(|err| de::Error::custom(format!("failed to parse {s:?} as a U256: {err}")))
    }
}

impl SerializeAs<ethereum_types::U256> for U256 {
    fn serialize_as<S: Serializer>(
        value: &ethereum_types::U256,
        serializer: S,
    ) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&value.to_string())
    }
}
