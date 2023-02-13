use {
    crate::domain::eth,
    serde::{de, Deserializer},
    serde_with::DeserializeAs,
};

/// Serialize and deserialize [`eth::ChainId`] values.
#[derive(Debug)]
pub struct ChainId;

impl<'de> DeserializeAs<'de, eth::ChainId> for ChainId {
    fn deserialize_as<D: Deserializer<'de>>(deserializer: D) -> Result<eth::ChainId, D::Error> {
        let value = super::U256::deserialize_as(deserializer)?;
        eth::ChainId::new(value).map_err(de::Error::custom)
    }
}
