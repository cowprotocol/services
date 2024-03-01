use {
    self::trade::ClearingPrices,
    super::auction,
    crate::{
        boundary,
        domain::{
            competition::{self, order},
            eth::{self, TokenAddress},
        },
        infra::{
            blockchain::{self, Ethereum},
            simulator,
            solver::Solver,
            Simulator,
        },
    },
    futures::future::try_join_all,
    itertools::Itertools,
    std::collections::{BTreeSet, HashMap},
    thiserror::Error,
};

pub mod fee;
pub mod interaction;
pub mod scoring;
pub mod settlement;
pub mod trade;

pub use {interaction::Interaction, settlement::Settlement, trade::Trade};

// TODO Add a constructor and ensure that the clearing prices are included for
// each trade
/// A solution represents a set of orders which the solver has found an optimal
/// way to settle. A [`Solution`] is generated by a solver as a response to a
/// [`competition::Auction`]. See also [`settlement::Settlement`].
#[derive(Clone)]
pub struct Solution {
    id: Id,
    trades: Vec<Trade>,
    prices: HashMap<eth::TokenAddress, eth::U256>,
    interactions: Vec<Interaction>,
    solver: Solver,
    score: SolverScore,
    weth: eth::WethAddress,
}

impl Solution {
    pub fn new(
        id: Id,
        trades: Vec<Trade>,
        prices: HashMap<eth::TokenAddress, eth::U256>,
        interactions: Vec<Interaction>,
        solver: Solver,
        score: SolverScore,
        weth: eth::WethAddress,
    ) -> Result<Self, SolutionError> {
        let solution = Self {
            id,
            trades,
            prices,
            interactions,
            solver,
            score,
            weth,
        };

        // Check that the solution includes clearing prices for all user trades.
        if solution.user_trades().any(|trade| {
            solution.clearing_price(trade.order().sell.token).is_none()
                || solution.clearing_price(trade.order().buy.token).is_none()
        }) {
            return Err(SolutionError::InvalidClearingPrices);
        }

        // Apply protocol fees
        let mut trades = Vec::with_capacity(solution.trades.len());
        for trade in solution.trades {
            match &trade {
                Trade::Fulfillment(fulfillment) => match fulfillment.order().kind {
                    order::Kind::Market | order::Kind::Limit { .. } => {
                        let prices = ClearingPrices {
                            sell: solution.prices
                                [&fulfillment.order().sell.token.wrap(solution.weth)],
                            buy: solution.prices
                                [&fulfillment.order().buy.token.wrap(solution.weth)],
                        };
                        let fulfillment = fulfillment.with_protocol_fee(prices)?;
                        trades.push(Trade::Fulfillment(fulfillment))
                    }
                    order::Kind::Liquidity => {
                        trades.push(trade);
                    }
                },
                Trade::Jit(_) => trades.push(trade),
            }
        }
        Ok(Self { trades, ..solution })
    }

    /// The ID of this solution.
    pub fn id(&self) -> Id {
        self.id
    }

    /// Trades settled by this solution.
    pub fn trades(&self) -> &[Trade] {
        &self.trades
    }

    /// Interactions executed by this solution.
    pub fn interactions(&self) -> &[Interaction] {
        &self.interactions
    }

    /// The solver which generated this solution.
    pub fn solver(&self) -> &Solver {
        &self.solver
    }

    pub fn score(&self) -> &SolverScore {
        &self.score
    }

    /// JIT score calculation as per CIP38
    pub fn scoring(&self, prices: &auction::Prices) -> Result<eth::TokenAmount, ScoringError> {
        let mut trades = Vec::with_capacity(self.trades.len());
        for trade in self.user_trades() {
            // Solver generated fulfillment does not include the fee in the executed amount
            // for sell orders.
            let executed = match trade.order().side {
                order::Side::Sell => (trade.executed().0 + trade.fee().0).into(),
                order::Side::Buy => trade.executed(),
            };
            let uniform_prices = ClearingPrices {
                sell: *self
                    .prices
                    .get(&trade.order().sell.token.wrap(self.weth))
                    .ok_or(SolutionError::InvalidClearingPrices)?,
                buy: *self
                    .prices
                    .get(&trade.order().buy.token.wrap(self.weth))
                    .ok_or(SolutionError::InvalidClearingPrices)?,
            };
            let custom_prices = scoring::CustomClearingPrices {
                sell: match trade.order().side {
                    order::Side::Sell => trade
                        .executed()
                        .0
                        .checked_mul(uniform_prices.sell)
                        .ok_or(Math::Overflow)?
                        .checked_div(uniform_prices.buy)
                        .ok_or(Math::DivisionByZero)?,
                    order::Side::Buy => trade.executed().0,
                },
                buy: match trade.order().side {
                    order::Side::Sell => trade.executed().0 + trade.fee().0,
                    order::Side::Buy => {
                        (trade.executed().0)
                            .checked_mul(uniform_prices.buy)
                            .ok_or(Math::Overflow)?
                            .checked_div(uniform_prices.sell)
                            .ok_or(Math::DivisionByZero)?
                            + trade.fee().0
                    }
                },
            };
            trades.push(scoring::Trade::new(
                trade.order().sell,
                trade.order().buy,
                trade.order().side,
                executed,
                custom_prices,
                trade.order().protocol_fees.clone(),
            ))
        }

        let scoring = scoring::Scoring::new(trades);
        Ok(scoring.score(prices)?)
    }

    /// Approval interactions necessary for encoding the settlement.
    pub async fn approvals(
        &self,
        eth: &Ethereum,
    ) -> Result<impl Iterator<Item = eth::allowance::Approval>, Error> {
        let settlement_contract = &eth.contracts().settlement();
        let allowances = try_join_all(self.allowances().map(|required| async move {
            eth.erc20(required.0.token)
                .allowance(settlement_contract.address().into(), required.0.spender)
                .await
                .map(|existing| (required, existing))
        }))
        .await?;
        let approvals = allowances.into_iter().filter_map(|(required, existing)| {
            required
                .approval(&existing)
                // As a gas optimization, we always approve the max amount possible. This minimizes
                // the number of approvals necessary, and therefore minimizes the approval fees over time. This is a
                // potential security issue, but its effects are minimized and only exploitable if
                // solvers use insecure contracts.
                .map(eth::allowance::Approval::max)
        });
        Ok(approvals)
    }

    /// An empty solution has no user trades and a score of 0.
    pub fn is_empty(&self) -> bool {
        self.user_trades().next().is_none()
    }

    /// Return the trades which fulfill non-liquidity auction orders. These are
    /// the orders placed by end users.
    fn user_trades(&self) -> impl Iterator<Item = &trade::Fulfillment> {
        self.trades.iter().filter_map(|trade| match trade {
            Trade::Fulfillment(fulfillment) => match fulfillment.order().kind {
                order::Kind::Market | order::Kind::Limit { .. } => Some(fulfillment),
                order::Kind::Liquidity => None,
            },
            Trade::Jit(_) => None,
        })
    }

    /// Return the allowances in a normalized form, where there is only one
    /// allowance per [`eth::allowance::Spender`], and they're ordered
    /// deterministically.
    fn allowances(&self) -> impl Iterator<Item = eth::allowance::Required> {
        let mut normalized = HashMap::new();
        // TODO: we need to carry the "internalize" flag with the allowances,
        // since we don't want to include approvals for interactions that are
        // meant to be internalized anyway.
        let allowances = self.interactions.iter().flat_map(Interaction::allowances);
        for allowance in allowances {
            let amount = normalized
                .entry((allowance.0.token, allowance.0.spender))
                .or_insert(eth::U256::zero());
            *amount = amount.saturating_add(allowance.0.amount);
        }
        normalized
            .into_iter()
            .map(|((token, spender), amount)| {
                eth::Allowance {
                    token,
                    spender,
                    amount,
                }
                .into()
            })
            .sorted()
    }

    /// Encode the solution into a [`Settlement`], which can be used to execute
    /// the solution onchain.
    pub async fn encode(
        self,
        auction: &competition::Auction,
        eth: &Ethereum,
        simulator: &Simulator,
    ) -> Result<Settlement, Error> {
        Settlement::encode(self, auction, eth, simulator).await
    }

    /// Token prices settled by this solution, expressed using an arbitrary
    /// reference unit chosen by the solver. These values are only
    /// meaningful in relation to each others.
    ///
    /// The rule which relates two prices for tokens X and Y is:
    /// ```
    /// amount_x * price_x = amount_y * price_y
    /// ```
    pub fn clearing_prices(&self) -> Result<Vec<eth::Asset>, Error> {
        let prices = self.prices.iter().map(|(&token, &amount)| eth::Asset {
            token,
            amount: amount.into(),
        });

        if self.user_trades().any(|trade| trade.order().buys_eth()) {
            // The solution contains an order which buys ETH. Solvers only produce solutions
            // for ERC20 tokens, while the driver adds special [`Interaction`]s to
            // wrap/unwrap the ETH tokens into WETH, and sends orders to the solver with
            // WETH instead of ETH. Once the driver receives the solution which fulfills an
            // ETH order, a clearing price for ETH needs to be added, equal to the
            // WETH clearing price.

            // If no order trades WETH, the WETH price is not necessary, only the ETH
            // price is needed. Remove the unneeded WETH price, which slightly reduces
            // gas used by the settlement.
            let mut prices = if self.user_trades().all(|trade| {
                trade.order().sell.token != self.weth.0 && trade.order().buy.token != self.weth.0
            }) {
                prices
                    .filter(|price| price.token != self.weth.0)
                    .collect_vec()
            } else {
                prices.collect_vec()
            };

            // Add a clearing price for ETH equal to WETH.
            prices.push(eth::Asset {
                token: eth::ETH_TOKEN,
                amount: self.prices[&self.weth.into()].to_owned().into(),
            });

            return Ok(prices);
        }

        // TODO: We should probably filter out all unused prices to save gas.

        Ok(prices.collect_vec())
    }

    /// Clearing price for the given token.
    pub fn clearing_price(&self, token: eth::TokenAddress) -> Option<eth::U256> {
        // The clearing price of ETH is equal to WETH.
        let token = token.wrap(self.weth);
        self.prices.get(&token).map(ToOwned::to_owned)
    }
}

impl std::fmt::Debug for Solution {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        f.debug_struct("Solution")
            .field("id", &self.id)
            .field("trades", &self.trades)
            .field("prices", &self.prices)
            .field("interactions", &self.interactions)
            .field("solver", &self.solver.name())
            .field("score", &self.score)
            .finish()
    }
}

/// Carries information how the score should be calculated.
#[derive(Debug, Clone)]
pub enum SolverScore {
    Solver(eth::U256),
    RiskAdjusted(f64),
    Surplus,
}
/// A unique solution ID. This ID is generated by the solver and only needs to
/// be unique within a single round of competition. This ID is only important in
/// the communication between the driver and the solver, and it is not used by
/// the protocol.
#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq)]
pub struct Id(pub u64);

impl From<u64> for Id {
    fn from(value: u64) -> Self {
        Self(value)
    }
}

impl From<Id> for u64 {
    fn from(value: Id) -> Self {
        value.0
    }
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("blockchain error: {0:?}")]
    Blockchain(#[from] blockchain::Error),
    #[error("boundary error: {0:?}")]
    Boundary(#[from] boundary::Error),
    #[error("simulation error: {0:?}")]
    Simulation(#[from] simulator::Error),
    #[error(
        "non bufferable tokens used: solution attempts to internalize tokens which are not trusted"
    )]
    NonBufferableTokensUsed(BTreeSet<TokenAddress>),
    #[error("invalid internalization: uninternalized solution fails to simulate")]
    FailingInternalization,
    #[error("insufficient solver account Ether balance, required {0:?}")]
    SolverAccountInsufficientBalance(eth::Ether),
    #[error("attempted to merge settlements generated by different solvers")]
    DifferentSolvers,
}

#[derive(Debug, thiserror::Error)]
pub enum SolutionError {
    #[error("invalid clearing prices")]
    InvalidClearingPrices,
    #[error(transparent)]
    ProtocolFee(#[from] fee::Error),
}

#[derive(Debug, thiserror::Error)]
pub enum ScoringError {
    #[error(transparent)]
    Solution(#[from] SolutionError),
    #[error(transparent)]
    Score(#[from] scoring::Error),
    #[error(transparent)]
    Math(#[from] Math),
}

#[derive(Debug, thiserror::Error)]
pub enum Math {
    #[error("overflow")]
    Overflow,
    #[error("division by zero")]
    DivisionByZero,
}
