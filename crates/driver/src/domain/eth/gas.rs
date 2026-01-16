use {
    super::{Ether, U256},
    alloy::eips::eip1559::calc_effective_gas_price,
    derive_more::{Display, From, Into},
    std::ops::{self, Add},
};

/// Gas amount in gas units.
///
/// The amount of Ether that is paid in transaction fees is proportional to this
/// amount as well as the transaction's [`EffectiveGasPrice`].
#[derive(Debug, Default, Display, Clone, Copy, Ord, Eq, PartialOrd, PartialEq, From, Into)]
pub struct Gas(pub U256);

impl From<u64> for Gas {
    fn from(value: u64) -> Self {
        Self(U256::from(value))
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
    base: Option<u64>,
}

impl GasPrice {
    /// Returns the estimated [`EffectiveGasPrice`] for the gas price estimate.
    pub fn effective(&self) -> EffectiveGasPrice {
        U256::from(calc_effective_gas_price(
            u128::try_from(self.max.0.0).expect("max fee per gas should fit in a u128"),
            u128::try_from(self.tip.0.0).expect("max priority fee per gas should fit in a u128"),
            self.base,
        ))
        .into()
    }

    pub fn max(&self) -> FeePerGas {
        self.max
    }

    pub fn tip(&self) -> FeePerGas {
        self.tip
    }

    pub fn base(&self) -> Option<u64> {
        self.base
    }

    pub fn new(max: FeePerGas, tip: FeePerGas, base: Option<u64>) -> Self {
        Self { max, tip, base }
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
        let value = value.0.0;
        Self {
            max: value.into(),
            tip: value.into(),
            base: u64::try_from(value).ok(),
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
        U256::from((f64::from(self.0.0) * rhs).ceil()).into()
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
        (self.0 * rhs.0.0).into()
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
