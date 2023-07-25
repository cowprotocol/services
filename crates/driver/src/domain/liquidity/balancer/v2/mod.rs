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
/// Internally it is represented as an exponent where the factor for scaling the
/// token is `10.pow(exponent)`.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct ScalingFactor(u8);

impl ScalingFactor {
    pub fn from_exponent(exponent: u8) -> Result<Self, InvalidScalingExponent> {
        if !(0..18).contains(&exponent) {
            return Err(InvalidScalingExponent);
        }
        Ok(Self(exponent))
    }

    pub fn exponent(&self) -> u8 {
        self.0
    }

    pub fn factor(&self) -> eth::U256 {
        let d = 18_u8
            .checked_sub(self.0)
            .expect("invariant guarantees this subtraction can't underflow");
        eth::U256::exp10(d.into())
    }
}

#[derive(Debug, thiserror::Error)]
#[error("scaling factor exponent must be in range [0, 18]")]
pub struct InvalidScalingExponent;
