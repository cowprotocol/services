use {
    crate::domain::eth,
    derive_more::{From, Into},
};

pub mod stable;
pub mod weighted;

/// A Balancer V2 pool ID.
///
/// Pool IDs are encoded as:
/// * 0..20: the address of the pool
/// * 20..22: the pool specialization
/// * 22..32: the pool nonce
#[derive(Clone, Copy, Debug, Eq, PartialEq, From, Into)]
pub struct Id(pub eth::H256);

impl Id {
    /// Extracts the pool address configured in the ID.
    pub fn address(&self) -> eth::ContractAddress {
        eth::H160::from_slice(&self.0[..20]).into()
    }
}

/// A Balancer V2 pool fee.
///
/// This is a fee factor represented as (value / 1e18).
#[derive(Clone, Copy, Debug, Default, Eq, Ord, PartialEq, PartialOrd)]
pub struct Fee(pub eth::U256);

impl Fee {
    /// Creates a new pool fee for the specified raw [`eth::U256`] value. This
    /// method expects a fee represented as `f * 1e18`. That is, a fee of 100%
    /// is created with `Fee::new(U256::exp10(18))`.
    pub fn from_raw(weight: eth::U256) -> Self {
        Self(weight)
    }

    /// Returns the fee as a raw [`eth::U256`] value as it is represented
    /// on-chain.
    pub fn as_raw(&self) -> eth::U256 {
        self.0
    }
}

/// A token scaling factor.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct ScalingFactor(eth::U256);

impl ScalingFactor {
    /// Creates a new scaling for the specified raw [`eth::U256`] value. This
    /// method expects a factor represented as `f * 1e18`. That is, a scaling
    /// factor of 1 is created with `ScalingFactor::new(U256::exp10(18))`.
    ///
    /// Returns `None` if the scaling factor is equal to 0.
    pub fn from_raw(factor: eth::U256) -> Result<Self, ZeroScalingFactor> {
        if factor.is_zero() {
            return Err(ZeroScalingFactor);
        }
        Ok(Self(factor))
    }

    /// Returns the scaling factor as a raw [`eth::U256`] value as it is
    /// represented on-chain.
    pub fn as_raw(&self) -> eth::U256 {
        self.0
    }
}

#[derive(Debug, thiserror::Error)]
#[error("scaling factor must be non-zero")]
pub struct ZeroScalingFactor;
