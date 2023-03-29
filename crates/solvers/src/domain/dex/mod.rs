//! Various solver implementations rely on quoting APIs from DEXs and DEX
//! aggregators. This domain module models the various types around quoting
//! single orders with DEXs and turning swaps into single order solutions.

use {
    crate::{
        domain::{auction, eth, order, solution},
        util,
    },
    ethereum_types::U256,
    std::fmt::{self, Debug, Formatter},
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
            .field("calldata", &util::fmt::Hex(&self.calldata))
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
    /// The gas guesstimate in gas units for the swap.
    ///
    /// This estimate is **not** expected to be accurate, and is purely
    /// indicative.
    pub gas: eth::Gas,
}

impl Swap {
    /// An approximation for the overhead of executing a trade in a settlement.
    const SETTLEMENT_OVERHEAD: u64 = 106_391;

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
    pub fn into_solution(
        self,
        order: order::Order,
        sell_price: auction::Price,
        gas: auction::GasPrice,
    ) -> Option<solution::Solution> {
        if (order.sell.token, order.buy.token) != (self.input.token, self.output.token) {
            return None;
        }

        let fee = if order.has_solver_fee() {
            // TODO: If the order has signed `fee` amount already, we should
            // discount it from the surplus fee. ATM, users would pay both a
            // full order fee as well as a solver computed fee. Note that this
            // is fine for now, since there is no way to create limit orders
            // with non-zero fees.
            solution::Fee::Surplus(
                sell_price.ether_value(eth::Ether(
                    self.gas
                        .0
                        .checked_add(Self::SETTLEMENT_OVERHEAD.into())?
                        .checked_mul(gas.0 .0)?,
                ))?,
            )
        } else {
            solution::Fee::Protocol
        };
        let surplus_fee = fee.surplus().unwrap_or_default();

        // Compute total executed sell and buy amounts accounting for solver
        // fees. That is, the total amount of sell tokens transferred into the
        // contract and the total buy tokens transferred out of the contract.
        let (sell, buy) = match order.side {
            order::Side::Buy => (
                self.input.amount.checked_add(surplus_fee)?,
                self.output.amount,
            ),
            order::Side::Sell => {
                // We want to collect fees in the sell token, so we need to sell
                // `fee` more than the DEX swap. However, we don't allow
                // transferring more than `order.sell.amount` (guaranteed by the
                // Smart Contract), so we need to cap our executed amount to the
                // order's limit sell amount and compute the executed buy amount
                // accordingly.
                let sell = self
                    .input
                    .amount
                    .checked_add(surplus_fee)?
                    .min(order.sell.amount);
                let buy = util::math::div_ceil(
                    sell.checked_sub(surplus_fee)?
                        .checked_mul(self.output.amount)?,
                    self.input.amount,
                )?;
                (sell, buy)
            }
        };

        // Check order's limit price is satisfied accounting for solver
        // specified fees.
        if order.sell.amount.checked_mul(buy)? < order.buy.amount.checked_mul(sell)? {
            return None;
        }

        let executed = match order.side {
            order::Side::Buy => buy,
            order::Side::Sell => sell.checked_sub(surplus_fee)?,
        };
        let allowance = self.allowance();
        Some(solution::Solution {
            prices: solution::ClearingPrices::new([
                (order.sell.token, buy),
                (order.buy.token, sell.checked_sub(surplus_fee)?),
            ]),
            trades: vec![solution::Trade::Fulfillment(solution::Fulfillment::new(
                order, executed, fee,
            )?)],
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
