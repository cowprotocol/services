pub mod u256_decimal;

use {
    anyhow::anyhow,
    primitive_types::U256,
    serde::{Deserialize, Deserializer, Serialize, Serializer},
};

#[derive(Copy, Clone, Debug, Hash, Eq, PartialEq, Ord, PartialOrd)]
pub struct NonZeroU256(U256);
impl TryFrom<U256> for NonZeroU256 {
    type Error = anyhow::Error;

    fn try_from(value: U256) -> Result<Self, Self::Error> {
        if value == U256::zero() {
            Err(anyhow!("Value cannot be zero!"))
        } else {
            Ok(Self(value))
        }
    }
}

impl Serialize for NonZeroU256 {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&self.0.to_string())
    }
}

impl<'de> Deserialize<'de> for NonZeroU256 {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        let u256_val = U256::from_dec_str(&s).map_err(serde::de::Error::custom)?;
        NonZeroU256::try_from(u256_val).map_err(serde::de::Error::custom)
    }
}

impl TryFrom<u128> for NonZeroU256 {
    type Error = anyhow::Error;

    fn try_from(value: u128) -> Result<Self, Self::Error> {
        NonZeroU256::try_from(U256::from(value))
    }
}

impl Default for NonZeroU256 {
    fn default() -> Self {
        Self(U256::one())
    }
}

impl Into<U256> for NonZeroU256 {
    fn into(self) -> U256 {
        self.0
    }
}

impl NonZeroU256 {
    pub fn one() -> Self {
        Self(U256::one())
    }

    pub fn get(&self) -> U256 {
        self.0
    }
}
