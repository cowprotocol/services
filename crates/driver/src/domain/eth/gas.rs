use {
    super::{Ether, U256},
    derive_more::{Display, From, Into},
    std::{ops, ops::Add},
};

/// Gas amount in gas units.
///
/// The amount of Ether that is paid in transaction fees is proportional to this
/// amount as well as the transaction's [`EffectiveGasPrice`].
#[derive(Debug, Default, Display, Clone, Copy, Ord, Eq, PartialOrd, PartialEq, From, Into)]
pub struct Gas(pub U256);

impl From<u64> for Gas {
    fn from(value: u64) -> Self {
        Self(value.into())
    }
}

impl Add for Gas {
    type Output = Self;

    fn add(self, rhs: Self) -> Self::Output {
        Self(self.0 + rhs.0)
    }
}

/// An EIP-1559 gas price estimate.
///
/// https://eips.ethereum.org/EIPS/eip-1559#specification
#[derive(Debug, Clone, Copy)]
pub struct GasPrice {
    /// The maximum total fee that should be charged.
    max: FeePerGas,
    /// The maximum priority fee (i.e. the tip to the block proposer) that
    /// can be charged.
    tip: FeePerGas,
    /// The current base gas price that will be charged to all accounts on the
    /// next block.
    base: FeePerGas,
}

impl GasPrice {
    /// Returns the estimated [`EffectiveGasPrice`] for the gas price estimate.
    pub fn effective(&self) -> EffectiveGasPrice {
        U256::from(self.max)
            .min(U256::from(self.base).saturating_add(self.tip.into()))
            .into()
    }

    pub fn max(&self) -> FeePerGas {
        self.max
    }

    pub fn tip(&self) -> FeePerGas {
        self.tip
    }

    /// Creates a new instance limiting maxFeePerGas to a reasonable multiple of
    /// the current base fee.
    pub fn new(max: FeePerGas, tip: FeePerGas, base: FeePerGas) -> Self {
        // We multiply a fixed factor of the current base fee per
        // gas, which is chosen to be the maximum possible increase to the base
        // fee (max 12.5% per block) over 12 blocks, also including the "tip".
        const MAX_FEE_FACTOR: f64 = 4.2;
        Self {
            max: FeePerGas(std::cmp::min(
                max.0,
                base.mul_ceil(MAX_FEE_FACTOR).add(tip).0,
            )),
            tip,
            base,
        }
    }
}

/// Implements multiplication of a gas price by a floating point number.
/// This is equivalent to multiplying the `tip` and `max`
impl std::ops::Mul<f64> for GasPrice {
    type Output = Self;

    fn mul(self, rhs: f64) -> Self::Output {
        Self {
            max: self.max.mul_ceil(rhs),
            tip: self.tip.mul_ceil(rhs),
            base: self.base,
        }
    }
}

impl From<EffectiveGasPrice> for GasPrice {
    fn from(value: EffectiveGasPrice) -> Self {
        let value = value.0 .0;
        Self {
            max: value.into(),
            tip: value.into(),
            base: value.into(),
        }
    }
}

/// The amount of ETH to pay as fees for a single unit of gas. This is
/// `{max,max_priority,base}_fee_per_gas` as defined by EIP-1559.
///
/// https://eips.ethereum.org/EIPS/eip-1559#specification
#[derive(Debug, Clone, Copy, Ord, Eq, PartialEq, PartialOrd)]
pub struct FeePerGas(pub Ether);

impl FeePerGas {
    /// Multiplies this fee by the given floating point number, rounding up.
    fn mul_ceil(self, rhs: f64) -> Self {
        U256::from_f64_lossy((self.0 .0.to_f64_lossy() * rhs).ceil()).into()
    }
}

impl From<U256> for FeePerGas {
    fn from(value: U256) -> Self {
        Self(value.into())
    }
}

impl ops::Add<FeePerGas> for FeePerGas {
    type Output = FeePerGas;

    fn add(self, rhs: FeePerGas) -> Self::Output {
        FeePerGas(self.0 + rhs.0)
    }
}

impl From<FeePerGas> for U256 {
    fn from(value: FeePerGas) -> Self {
        value.0.into()
    }
}

impl ops::Mul<FeePerGas> for Gas {
    type Output = Ether;

    fn mul(self, rhs: FeePerGas) -> Self::Output {
        (self.0 * rhs.0 .0).into()
    }
}

/// The `effective_gas_price` as defined by EIP-1559.
///
/// https://eips.ethereum.org/EIPS/eip-1559#specification
#[derive(Debug, Clone, Copy)]
pub struct EffectiveGasPrice(pub Ether);

impl From<U256> for EffectiveGasPrice {
    fn from(value: U256) -> Self {
        Self(value.into())
    }
}

impl From<EffectiveGasPrice> for U256 {
    fn from(value: EffectiveGasPrice) -> Self {
        value.0.into()
    }
}
