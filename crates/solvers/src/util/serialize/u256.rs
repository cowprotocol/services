use {
    serde::{de, Deserialize, Deserializer},
    serde_with::DeserializeAs,
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
