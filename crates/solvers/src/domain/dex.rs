//! Various solver implementations rely on quoting APIs from DEXs and DEX
//! aggregators. This domain module models the various types around quoting
//! single orders with DEXs and turning swaps into single order solutions.

use {
    crate::{
        domain::{eth, order, solution},
        util::fmt,
    },
    ethereum_types::U256,
    std::fmt::{Debug, Formatter},
};

/// An order for quoting with an external DEX or DEX aggregator. This is a
/// simplified representation of a CoW Protocol order.
#[derive(Debug)]
pub struct Order {
    pub sell: eth::TokenAddress,
    pub buy: eth::TokenAddress,
    pub side: order::Side,
    pub amount: Amount,
}

impl Order {
    /// Returns the order swapped amount as an asset. The token associated with
    /// the asset is dependent on the side of the DEX order.
    pub fn amount(&self) -> eth::Asset {
        eth::Asset {
            token: match self.side {
                order::Side::Buy => self.buy,
                order::Side::Sell => self.sell,
            },
            amount: self.amount.0,
        }
    }
}

impl From<order::Order> for Order {
    fn from(value: order::Order) -> Self {
        Self {
            sell: value.sell.token,
            buy: value.buy.token,
            side: value.side,
            amount: Amount(match value.side {
                order::Side::Buy => value.buy.amount,
                order::Side::Sell => value.buy.amount,
            }),
        }
    }
}

/// A slippage tolerance (as a factor) that can be applied to token amounts.
#[derive(Clone, Copy, Debug)]
pub struct Slippage(f64);

impl Slippage {
    /// Creates a new slippage value. The value represents the slippage
    /// tolerance as a factor (so 0.05 means that up to 5% slippage tolerance is
    /// accepted, i.e. a swap can return 5% less than promised). Returns `None`
    /// if an invalid value outside of the range [0.0, 1.0] is specified.
    pub fn new(value: f64) -> Option<Self> {
        (0.0..=1.0).contains(&value).then_some(Self(value))
    }

    /// Adds slippage to the specified token amount. This can be used to account
    /// for negative slippage in a sell amount.
    pub fn add(&self, amount: U256) -> U256 {
        U256::from_f64_lossy((1.0 + self.0) * amount.to_f64_lossy())
    }

    /// Subtracts slippage to the specified token amount. This can be used to
    /// account for negative slippage in a buy amount.
    pub fn sub(&self, amount: U256) -> U256 {
        U256::from_f64_lossy((1.0 - self.0) * amount.to_f64_lossy())
    }
}

/// An on-chain Ethereum call for executing a DEX swap.
pub struct Call {
    /// The address that gets called on-chain.
    pub to: eth::ContractAddress,
    /// The associated calldata for the on-chain call.
    pub calldata: Vec<u8>,
}

impl Debug for Call {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        f.debug_struct("Call")
            .field("to", &self.to)
            .field("calldata", &fmt::Hex(&self.calldata))
            .finish()
    }
}

/// A DEX swap.
#[derive(Debug)]
pub struct Swap {
    /// The Ethereum call for executing the swap.
    pub call: Call,
    /// The expected input asset for the swap. The executed input may end up
    /// being different because of slippage.
    pub input: eth::Asset,
    /// The expected output asset for the swap. The executed output may end up
    /// being different because of slippage.
    pub output: eth::Asset,
    /// The minimum allowance that is required for executing the swap.
    pub allowance: Allowance,
}

impl Swap {
    pub fn allowance(&self) -> solution::Allowance {
        solution::Allowance {
            spender: self.allowance.spender.0,
            asset: eth::Asset {
                token: self.input.token,
                amount: self.allowance.amount.0,
            },
        }
    }
}

/// A swap allowance.
#[derive(Debug)]
pub struct Allowance {
    /// The spender address that requires an allowance in order to execute a
    /// swap.
    pub spender: eth::ContractAddress,
    /// The amount, in tokens, of the required allowance.
    pub amount: Amount,
}

/// A token amount.
#[derive(Debug)]
pub struct Amount(U256);

impl Amount {
    pub fn new(value: U256) -> Self {
        Self(value)
    }
}
