use {
    anyhow::anyhow,
    primitive_types::U256,
    serde::{Deserialize, Serialize},
};

#[derive(Copy, Clone, Deserialize, Serialize, Debug, Hash, Eq, PartialEq)]
#[serde(try_from = "U256", into = "U256")]
pub struct NonZeroU256(#[serde(with = "u256_decimal")] U256);
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
