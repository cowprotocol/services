use super::eth;

/// A token swap. Specifies how much of one token should be converted to another
/// token.
#[derive(Debug, Clone, Copy)]
pub struct Swap {
    /// The token to swap from.
    pub from: FromToken,
    /// The token to swap into.
    pub to: ToToken,
    /// The amount to swap.
    pub amount: FromAmount,
}

/// The token to convert from.
#[derive(Debug, Clone, Copy)]
pub struct FromToken(eth::TokenAddress);

impl From<FromToken> for eth::H160 {
    fn from(value: FromToken) -> Self {
        value.0 .0
    }
}

impl From<eth::H160> for FromToken {
    fn from(value: eth::H160) -> Self {
        Self(eth::TokenAddress(value))
    }
}

impl From<eth::TokenAddress> for FromToken {
    fn from(value: eth::TokenAddress) -> Self {
        Self(value)
    }
}

/// The token to convert into.
#[derive(Debug, Clone, Copy)]
pub struct ToToken(eth::TokenAddress);

impl From<ToToken> for eth::H160 {
    fn from(value: ToToken) -> Self {
        value.0 .0
    }
}

impl From<eth::H160> for ToToken {
    fn from(value: eth::H160) -> Self {
        Self(eth::TokenAddress(value))
    }
}

/// Amount of [`FromToken`].
#[derive(Debug, Clone, Copy)]
pub struct FromAmount(eth::U256);

impl From<FromAmount> for eth::U256 {
    fn from(value: FromAmount) -> Self {
        value.0
    }
}

impl From<eth::U256> for FromAmount {
    fn from(value: eth::U256) -> Self {
        Self(value)
    }
}

/// Amount of [`ToToken`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct ToAmount(eth::U256);

impl From<ToAmount> for eth::U256 {
    fn from(value: ToAmount) -> Self {
        value.0
    }
}

impl From<eth::U256> for ToAmount {
    fn from(value: eth::U256) -> Self {
        Self(value)
    }
}
