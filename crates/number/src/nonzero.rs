use {
    anyhow::anyhow,
    primitive_types::U256 as ZeroU256,
    serde::{Deserialize, Deserializer, Serialize, Serializer},
};

#[derive(Copy, Clone, Debug, Hash, Eq, PartialEq, Ord, PartialOrd)]
pub struct U256(ZeroU256);
impl TryFrom<ZeroU256> for U256 {
    type Error = anyhow::Error;

    fn try_from(value: ZeroU256) -> Result<Self, Self::Error> {
        if value == ZeroU256::zero() {
            Err(anyhow!("Value cannot be zero!"))
        } else {
            Ok(Self(value))
        }
    }
}

impl Serialize for U256 {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&self.0.to_string())
    }
}

impl<'de> Deserialize<'de> for U256 {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        let u256_val = ZeroU256::from_dec_str(&s).map_err(serde::de::Error::custom)?;
        U256::try_from(u256_val).map_err(serde::de::Error::custom)
    }
}

impl TryFrom<u128> for U256 {
    type Error = anyhow::Error;

    fn try_from(value: u128) -> Result<Self, Self::Error> {
        U256::try_from(ZeroU256::from(value))
    }
}

impl Default for U256 {
    fn default() -> Self {
        Self(ZeroU256::one())
    }
}

impl From<U256> for ZeroU256 {
    fn from(val: U256) -> Self {
        val.0
    }
}

impl U256 {
    pub fn one() -> Self {
        Self(ZeroU256::one())
    }

    pub fn get(&self) -> ZeroU256 {
        self.0
    }
}
