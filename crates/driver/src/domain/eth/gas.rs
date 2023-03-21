use {
    super::{Ether, U256},
    std::ops,
};

/// Gas amount.
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
    pub max: MaxFeePerGas,
    /// The maximum priority fee (i.e. the tip to the block proposer) that
    /// can be charged.
    pub tip: MaxPriorityFeePerGas,
    /// The current base gas price that will be charged to all accounts on the
    /// next block.
    pub base: BaseFeePerGas,
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

/// The `max_fee_per_gas` as defined by EIP-1559.
///
/// https://eips.ethereum.org/EIPS/eip-1559#specification
#[derive(Debug, Clone, Copy)]
pub struct MaxFeePerGas(pub Ether);

impl From<U256> for MaxFeePerGas {
    fn from(value: U256) -> Self {
        Self(value.into())
    }
}

impl From<MaxFeePerGas> for U256 {
    fn from(value: MaxFeePerGas) -> Self {
        value.0.into()
    }
}

impl ops::Mul<MaxFeePerGas> for Gas {
    type Output = Ether;

    fn mul(self, rhs: MaxFeePerGas) -> Self::Output {
        (self.0 * rhs.0 .0).into()
    }
}

/// The `max_priority_fee_per_gas` as defined by EIP-1559.
///
/// https://eips.ethereum.org/EIPS/eip-1559#specification
#[derive(Debug, Clone, Copy)]
pub struct MaxPriorityFeePerGas(pub Ether);

impl From<U256> for MaxPriorityFeePerGas {
    fn from(value: U256) -> Self {
        Self(value.into())
    }
}

impl From<MaxPriorityFeePerGas> for U256 {
    fn from(value: MaxPriorityFeePerGas) -> Self {
        value.0.into()
    }
}

/// The `base_fee_per_gas` as defined by EIP-1559.
///
/// https://eips.ethereum.org/EIPS/eip-1559#specification
#[derive(Debug, Clone, Copy)]
pub struct BaseFeePerGas(pub Ether);

impl From<U256> for BaseFeePerGas {
    fn from(value: U256) -> Self {
        Self(value.into())
    }
}

impl From<BaseFeePerGas> for U256 {
    fn from(value: BaseFeePerGas) -> Self {
        value.0.into()
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
