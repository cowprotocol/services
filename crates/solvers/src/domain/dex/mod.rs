//! Various solver implementations rely on quoting APIs from DEXs and DEX
//! aggregators. This domain module models the various types around quoting
//! single orders with DEXs and turning swaps into single order solutions.

use {
    crate::{
        domain::{self, auction, eth, order, solution},
        infra,
        util,
    },
    alloy::primitives::{Address, U256},
    std::fmt::{self, Debug, Formatter},
};

pub mod minimum_surplus;
mod shared;
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
    pub owner: eth::Address,
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
            owner: order.owner(),
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
    pub to: Address,
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
    /// The Ethereum calls for executing the swap.
    pub calls: Vec<Call>,
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
    pub fn allowance(&self) -> solution::Allowance {
        solution::Allowance {
            spender: self.allowance.spender,
            asset: eth::Asset {
                token: self.input.token,
                amount: self.allowance.amount.0,
            },
        }
    }

    /// Constructs a single order `solution::Solution` for this swap. Returns
    /// `None` if the swap is not valid for the specified order.
    pub async fn into_solution(
        self,
        order: order::Order,
        gas_price: auction::GasPrice,
        sell_token: Option<auction::Price>,
        simulator: &infra::dex::Simulator,
        gas_offset: eth::Gas,
    ) -> Option<solution::Solution> {
        let gas = if order.class == order::Class::Limit {
            match simulator.gas(order.owner(), &self).await {
                Ok(value) => value,
                Err(infra::dex::simulator::Error::SettlementContractIsOwner) => self.gas,
                Err(err) => {
                    tracing::warn!(?err, "gas simulation failed");
                    return None;
                }
            }
        } else {
            // We are fine with just using heuristic gas for market orders,
            // since it doesn't really play a role in the final solution.
            self.gas
        };

        let allowance = self.allowance();
        let interactions = self
            .calls
            .into_iter()
            .map(|call| {
                solution::Interaction::Custom(solution::CustomInteraction {
                    target: call.to,
                    value: eth::Ether::default(),
                    calldata: call.calldata,
                    inputs: vec![self.input],
                    outputs: vec![self.output],
                    internalize: false,
                    allowances: vec![allowance.clone()],
                })
            })
            .collect();

        solution::Single {
            order,
            input: self.input,
            output: self.output,
            interactions,
            gas,
            wrappers: vec![],
        }
        .into_dex_solution(gas_price, sell_token, gas_offset)
    }

    pub fn satisfies(&self, order: &domain::order::Order) -> bool {
        self.output
            .amount
            .widening_mul::<_, _, 512, 8>(order.sell.amount)
            >= self.input.amount.widening_mul(order.buy.amount)
    }

    pub fn satisfies_with_minimum_surplus(
        &self,
        order: &domain::order::Order,
        minimum_surplus: &minimum_surplus::MinimumSurplus,
    ) -> bool {
        let required_buy_amount = minimum_surplus.add(order.buy.amount);
        self.output
            .amount
            .widening_mul::<_, _, 512, 8>(order.sell.amount)
            >= self.input.amount.widening_mul(required_buy_amount)
    }
}

/// A swap allowance.
#[derive(Debug)]
pub struct Allowance {
    /// The spender address that requires an allowance in order to execute a
    /// swap.
    pub spender: Address,
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
