use {
    alloy::primitives::U256,
    anyhow::Context,
    serde::{Deserialize, Deserializer, Serialize, Serializer},
    std::fmt::{self, Display, Formatter},
};

#[derive(Copy, Clone, Debug, Hash, Eq, PartialEq, Ord, PartialOrd)]
pub struct NonZeroU256(U256);

impl NonZeroU256 {
    pub const MAX: Self = NonZeroU256(U256::MAX);
    pub const ONE: Self = NonZeroU256(U256::ONE);

    pub fn new(value: U256) -> Option<Self> {
        (!value.is_zero()).then_some(Self(value))
    }

    pub fn get(self) -> U256 {
        self.0
    }
}

impl TryFrom<U256> for NonZeroU256 {
    type Error = anyhow::Error;

    fn try_from(value: U256) -> Result<Self, Self::Error> {
        Self::new(value).context("Value cannot be zero!")
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
        let u256 = U256::deserialize(deserializer)?;
        NonZeroU256::try_from(u256).map_err(serde::de::Error::custom)
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
        Self::ONE
    }
}

impl From<NonZeroU256> for U256 {
    fn from(val: NonZeroU256) -> Self {
        val.0
    }
}

impl Display for NonZeroU256 {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        Display::fmt(&self.0, f)
    }
}
