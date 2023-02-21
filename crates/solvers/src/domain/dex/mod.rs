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

pub mod slippage;

pub use self::slippage::Slippage;

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
    pub fn new(order: &order::Order) -> Self {
        Self {
            sell: order.sell.token,
            buy: order.buy.token,
            side: order.side,
            amount: Amount(match order.side {
                order::Side::Buy => order.buy.amount,
                order::Side::Sell => order.sell.amount,
            }),
        }
    }

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

    /// Constructs a single order `solution::Solution` for this swap. Returns
    /// `None` if the swap is not valid for the specified order.
    pub fn into_solution(self, order: order::Order) -> Option<solution::Solution> {
        if !self.matches_order(&order) || !self.respects_price(&order) {
            return None;
        }

        let allowance = self.allowance();
        Some(solution::Solution {
            prices: solution::ClearingPrices::new([
                (self.input.token, self.output.amount),
                (self.output.token, self.input.amount),
            ]),
            trades: vec![solution::Trade::Fulfillment(solution::Fulfillment::fill(
                order,
            ))],
            interactions: vec![solution::Interaction::Custom(solution::CustomInteraction {
                target: self.call.to.0,
                value: eth::Ether::default(),
                calldata: self.call.calldata,
                inputs: vec![self.input],
                outputs: vec![self.output],
                internalize: false,
                allowances: vec![allowance],
            })],
        })
    }

    fn matches_order(&self, order: &order::Order) -> bool {
        let (swap_amount, order_amount) = match order.side {
            order::Side::Buy => (self.output.amount, order.buy.amount),
            order::Side::Sell => (self.input.amount, order.sell.amount),
        };

        let correct_tokens =
            (order.sell.token, order.buy.token) == (self.input.token, self.output.token);
        let correct_amount = match order.partially_fillable {
            true => swap_amount <= order_amount,
            false => swap_amount == order_amount,
        };

        correct_tokens && correct_amount
    }

    fn respects_price(&self, order: &order::Order) -> bool {
        // Note the use of checked multiplication - this is consistent with the
        // on-chain limit price check.
        let sell = order.sell.amount.checked_mul(self.output.amount);
        let buy = order.buy.amount.checked_mul(self.input.amount);
        matches!((sell, buy), (Some(sell), Some(buy)) if sell >= buy)
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

    pub fn get(&self) -> U256 {
        self.0
    }
}
