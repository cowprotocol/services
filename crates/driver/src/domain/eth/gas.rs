use {
    super::{Ether, U256},
    std::ops,
};

/// Gas amount in gas units.
///
/// The amount of Ether that is paid in transaction fees is proportional to this
/// amount as well as the transaction's [`EffectiveGasPrice`].
#[derive(Debug, Default, Clone, Copy)]
pub struct Gas(pub U256);

impl From<U256> for Gas {
    fn from(value: U256) -> Self {
        Self(value)
    }
}

impl From<u64> for Gas {
    fn from(value: u64) -> Self {
        Self(value.into())
    }
}

impl From<Gas> for U256 {
    fn from(value: Gas) -> Self {
        value.0
    }
}

/// An EIP-1559 gas price estimate.
///
/// https://eips.ethereum.org/EIPS/eip-1559#specification
#[derive(Debug, Clone, Copy)]
pub struct GasPrice {
    /// The maximum total fee that should be charged.
    pub max: FeePerGas,
    /// The maximum priority fee (i.e. the tip to the block proposer) that
    /// can be charged.
    pub tip: FeePerGas,
    /// The current base gas price that will be charged to all accounts on the
    /// next block.
    pub base: FeePerGas,
}

impl GasPrice {
    /// Returns the estimated [`EffectiveGasPrice`] for the gas price estimate.
    pub fn effective(&self) -> EffectiveGasPrice {
        U256::from(self.max)
            .min(U256::from(self.base).saturating_add(self.tip.into()))
            .into()
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

/// A measurement of an of Ether to pay as fees for a single gas unit. This is
/// `{max,max_priority,base}_fee_per_gas` as defined by EIP-1559.
#[derive(Debug, Clone, Copy)]
pub struct FeePerGas(pub Ether);

impl From<U256> for FeePerGas {
    fn from(value: U256) -> Self {
        Self(value.into())
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
