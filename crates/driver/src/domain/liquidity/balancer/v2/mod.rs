use crate::domain::eth;

pub mod stable;
pub mod weighted;

/// A Balancer V2 pool ID.
///
/// Pool IDs are encoded as:
/// * 0..20: the address of the pool
/// * 20..22: the pool specialization
/// * 22..32: the pool nonce
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Id(pub eth::H256);

impl Id {
    /// Extracts the pool address configured in the ID.
    pub fn address(&self) -> eth::ContractAddress {
        eth::H160::from_slice(&self.0[..20]).into()
    }
}

impl From<eth::H256> for Id {
    fn from(value: eth::H256) -> Self {
        Self(value)
    }
}

impl From<Id> for eth::H256 {
    fn from(value: Id) -> Self {
        value.0
    }
}

/// A Balancer V2 pool fee.
///
/// This is a fee factor represented as (value / 1e18).
#[derive(Clone, Copy, Debug, Default, Eq, Ord, PartialEq, PartialOrd)]
pub struct Fee(pub eth::U256);

impl From<eth::U256> for Fee {
    fn from(value: eth::U256) -> Self {
        Self(value)
    }
}

impl From<Fee> for eth::U256 {
    fn from(value: Fee) -> Self {
        value.0
    }
}

/// A token scaling factor.
///
/// Scaling factors are rational numbers represented as (value / 1e18).
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct ScalingFactor(eth::U256);

impl ScalingFactor {
    pub fn new(factor: eth::U256) -> Result<Self, InvalidScalingFactor> {
        if factor.is_zero() {
            return Err(InvalidScalingFactor);
        }
        Ok(Self(factor))
    }
}

impl From<ScalingFactor> for eth::U256 {
    fn from(value: ScalingFactor) -> Self {
        value.0
    }
}

#[derive(Debug, thiserror::Error)]
#[error("scaling factor must be non-zero")]
pub struct InvalidScalingFactor;
