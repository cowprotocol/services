use {
    crate::{
        domain::{auction, eth, liquidity, order},
        util,
    },
    ethereum_types::{Address, U256},
    std::collections::HashMap,
};

/// A solution to an auction.
#[derive(Debug, Default)]
pub struct Solution {
    pub prices: ClearingPrices,
    pub trades: Vec<Trade>,
    pub interactions: Vec<Interaction>,
    pub score: Score,
}

impl Solution {
    pub fn with_score(self, score: Score) -> Self {
        Self { score, ..self }
    }
}

/// A solution for a settling a single order.
pub struct Single {
    /// The order included in this single order solution.
    pub order: order::Order,
    /// The total input to the swap for executing a single order.
    pub input: eth::Asset,
    /// The total output of the swap for executing a single order.
    pub output: eth::Asset,
    /// The swap interactions for the single order settlement.
    pub interactions: Vec<Interaction>,
    /// The estimated gas needed for swapping the sell amount to buy amount.
    pub gas: eth::Gas,
}

impl Single {
    /// An approximation for the overhead of executing a trade in a settlement.
    const SETTLEMENT_OVERHEAD: u64 = 106_391;

    /// Creates a full solution for a single order solution given gas and sell
    /// token prices.
    pub fn into_solution(
        self,
        gas_price: auction::GasPrice,
        sell_token: Option<auction::Price>,
    ) -> Option<Solution> {
        let Self {
            order,
            input,
            output,
            interactions,
            gas: swap,
        } = self;

        if (order.sell.token, order.buy.token) != (input.token, output.token) {
            return None;
        }

        let fee = if order.solver_determines_fee() {
            // TODO: If the order has signed `fee` amount already, we should
            // discount it from the surplus fee. ATM, users would pay both a
            // full order fee as well as a solver computed fee. Note that this
            // is fine for now, since there is no way to create limit orders
            // with non-zero fees.
            Fee::Surplus(
                sell_token?.ether_value(eth::Ether(
                    swap.0
                        .checked_add(Self::SETTLEMENT_OVERHEAD.into())?
                        .checked_mul(gas_price.0 .0)?,
                ))?,
            )
        } else {
            Fee::Protocol
        };
        let surplus_fee = fee.surplus().unwrap_or_default();

        // Compute total executed sell and buy amounts accounting for solver
        // fees. That is, the total amount of sell tokens transferred into the
        // contract and the total buy tokens transferred out of the contract.
        let (sell, buy) = match order.side {
            order::Side::Buy => (input.amount.checked_add(surplus_fee)?, output.amount),
            order::Side::Sell => {
                // We want to collect fees in the sell token, so we need to sell
                // `fee` more than the DEX swap. However, we don't allow
                // transferring more than `order.sell.amount` (guaranteed by the
                // Smart Contract), so we need to cap our executed amount to the
                // order's limit sell amount and compute the executed buy amount
                // accordingly.
                let sell = input
                    .amount
                    .checked_add(surplus_fee)?
                    .min(order.sell.amount);
                let buy = util::math::div_ceil(
                    sell.checked_sub(surplus_fee)?.checked_mul(output.amount)?,
                    input.amount,
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
        Some(Solution {
            prices: ClearingPrices::new([
                (order.sell.token, buy),
                (order.buy.token, sell.checked_sub(surplus_fee)?),
            ]),
            trades: vec![Trade::Fulfillment(Fulfillment::new(order, executed, fee)?)],
            interactions,
            score: Default::default(),
        })
    }
}

/// A set of uniform clearing prices. They are represented as a mapping of token
/// addresses to price in an arbitrarily denominated price.
#[derive(Debug, Default)]
pub struct ClearingPrices(pub HashMap<eth::TokenAddress, U256>);

impl ClearingPrices {
    /// Creates a new set of clearing prices.
    pub fn new(prices: impl IntoIterator<Item = (eth::TokenAddress, U256)>) -> Self {
        Self(prices.into_iter().collect())
    }
}

/// A trade which executes an order as part of this solution.
#[derive(Debug)]
pub enum Trade {
    Fulfillment(Fulfillment),
    Jit(JitTrade),
}

/// A traded order within a solution.
#[derive(Debug)]
pub struct Fulfillment {
    order: order::Order,
    executed: U256,
    fee: Fee,
}

impl Fulfillment {
    /// Creates a new order filled to the specified amount. Returns `None` if
    /// the fill amount is incompatible with the order.
    pub fn new(order: order::Order, executed: U256, fee: Fee) -> Option<Self> {
        if matches!(fee, Fee::Surplus(_)) != order.solver_determines_fee() {
            return None;
        }

        let (fill, full) = match order.side {
            order::Side::Buy => (order.buy.amount, executed),
            order::Side::Sell => (
                order.sell.amount,
                executed.checked_add(fee.surplus().unwrap_or_default())?,
            ),
        };
        if (!order.partially_fillable && full != fill) || (order.partially_fillable && full > fill)
        {
            return None;
        }

        Some(Self {
            order,
            executed,
            fee,
        })
    }

    /// Creates a new trade for a fully executed order.
    pub fn fill(order: order::Order) -> Option<Self> {
        let executed = match order.side {
            order::Side::Buy => order.buy.amount,
            order::Side::Sell => order.sell.amount,
        };
        Self::new(order, executed, Fee::Protocol)
    }

    /// Get a reference to the traded order.
    pub fn order(&self) -> &order::Order {
        &self.order
    }

    /// Returns the trade execution as an asset (token address and amount).
    pub fn executed(&self) -> eth::Asset {
        let token = match self.order.side {
            order::Side::Buy => self.order.buy.token,
            order::Side::Sell => self.order.sell.token,
        };

        eth::Asset {
            token,
            amount: self.executed,
        }
    }

    /// Returns the solver computed fee that was charged to the order as an
    /// asset (token address and amount). Returns `None` if the fulfillment
    /// does not include a solver computed fee.
    pub fn surplus_fee(&self) -> Option<eth::Asset> {
        Some(eth::Asset {
            token: self.order.sell.token,
            amount: self.fee.surplus()?,
        })
    }
}

/// The fee that is charged to a user for executing an order.
#[derive(Clone, Copy, Debug)]
pub enum Fee {
    /// A protocol computed fee.
    ///
    /// That is, the fee is charged from the order's `fee_amount` that is
    /// included in the auction being solved.
    Protocol,

    /// An additional surplus fee that is charged by the solver.
    Surplus(U256),
}

impl Fee {
    /// Returns the dynamic component for the fee.
    pub fn surplus(&self) -> Option<U256> {
        match self {
            Fee::Protocol => None,
            Fee::Surplus(fee) => Some(*fee),
        }
    }
}

/// A trade of an order that was created specifically for this solution
/// providing just-in-time liquidity for other regular orders.
#[derive(Debug)]
pub struct JitTrade {
    pub order: order::JitOrder,
    pub executed: U256,
}

/// An interaction that is required to execute a solution by acquiring liquidity
/// or running some custom logic.
#[derive(Debug)]
pub enum Interaction {
    Liquidity(LiquidityInteraction),
    Custom(CustomInteraction),
}

/// An interaction using input liquidity. This interaction will be encoded by
/// the driver.
#[derive(Debug)]
pub struct LiquidityInteraction {
    pub liquidity: liquidity::Liquidity,
    // TODO: Currently there is not type-level guarantee that `input` and
    // output` are valid for the specified liquidity.
    pub input: eth::Asset,
    pub output: eth::Asset,
    pub internalize: bool,
}

/// An arbitrary interaction returned by the solver, which needs to be executed
/// to fulfill the trade.
#[derive(Debug)]
pub struct CustomInteraction {
    pub target: Address,
    pub value: eth::Ether,
    pub calldata: Vec<u8>,
    /// Indicated whether the interaction should be internalized (skips its
    /// execution as an optimization). This is only allowed under certain
    /// conditions.
    pub internalize: bool,
    /// Documents inputs of the interaction to determine whether internalization
    /// is actually legal.
    pub inputs: Vec<eth::Asset>,
    /// Documents outputs of the interaction to determine whether
    /// internalization is actually legal.
    pub outputs: Vec<eth::Asset>,
    /// Allowances required to successfully execute the interaction.
    pub allowances: Vec<Allowance>,
}

/// Approval required to make some `[CustomInteraction]` possible.
#[derive(Debug)]
pub struct Allowance {
    pub spender: Address,
    pub asset: eth::Asset,
}

/// Represents the probability that a solution will be successfully settled.
type SuccessProbability = f64;

/// A score for a solution. The score is used to rank solutions.
#[derive(Debug, Clone)]
pub enum Score {
    /// The score value is provided as is from solver.
    /// Success probability is not incorporated into this value.
    Solver(U256),
    /// This option is used to indicate that the solver did not provide a score.
    /// Instead, the score should be computed by the protocol given the success
    /// probability.
    RiskAdjusted(SuccessProbability),
}

impl Default for Score {
    fn default() -> Self {
        Self::RiskAdjusted(1.0)
    }
}
