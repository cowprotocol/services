use {
    self::trade::{ClearingPrices, Fee, Fulfillment},
    super::auction,
    crate::{
        boundary,
        domain::{
            competition::{self, order},
            eth::{self, Flashloan, TokenAddress},
        },
        infra::{
            blockchain::{self, Ethereum},
            config::file::FeeHandler,
            simulator,
            solver::{ManageNativeToken, Solver},
            Simulator,
        },
    },
    chrono::Utc,
    futures::future::try_join_all,
    itertools::Itertools,
    num::{BigRational, One},
    std::{
        collections::{hash_map::Entry, BTreeSet, HashMap, HashSet},
        sync::atomic::{AtomicU64, Ordering},
    },
    thiserror::Error,
};

pub mod encoding;
pub mod fee;
pub mod interaction;
pub mod scoring;
pub mod settlement;
pub mod slippage;
pub mod trade;

pub use {error::Error, interaction::Interaction, settlement::Settlement, trade::Trade};

type Prices = HashMap<eth::TokenAddress, eth::U256>;

// TODO Add a constructor and ensure that the clearing prices are included for
// each trade
/// A solution represents a set of orders which the solver has found an optimal
/// way to settle. A [`Solution`] is generated by a solver as a response to a
/// [`competition::Auction`]. See also [`settlement::Settlement`].
#[derive(Clone)]
pub struct Solution {
    id: Id,
    trades: Vec<Trade>,
    prices: Prices,
    pre_interactions: Vec<eth::Interaction>,
    interactions: Vec<Interaction>,
    post_interactions: Vec<eth::Interaction>,
    solver: Solver,
    weth: eth::WethAddress,
    gas: Option<eth::Gas>,
    flashloans: Vec<Flashloan>,
}

impl Solution {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        id: Id,
        mut trades: Vec<Trade>,
        prices: Prices,
        pre_interactions: Vec<eth::Interaction>,
        interactions: Vec<Interaction>,
        post_interactions: Vec<eth::Interaction>,
        solver: Solver,
        weth: eth::WethAddress,
        gas: Option<eth::Gas>,
        fee_handler: FeeHandler,
        surplus_capturing_jit_order_owners: &HashSet<eth::Address>,
        flashloans: Vec<Flashloan>,
    ) -> Result<Self, error::Solution> {
        // Surplus capturing JIT orders behave like Fulfillment orders. They capture
        // surplus, pay network fees and contribute to score of a solution.
        // To make sure that all the same logic and checks get applied we convert them
        // right away.
        for trade in &mut trades {
            let Trade::Jit(jit) = trade else { continue };
            if !surplus_capturing_jit_order_owners.contains(&jit.order().signature.signer) {
                continue;
            }

            *trade = Trade::Fulfillment(
                Fulfillment::new(
                    competition::Order {
                        uid: jit.order().uid,
                        kind: order::Kind::Limit,
                        side: jit.order().side,
                        sell: jit.order().sell,
                        buy: jit.order().buy,
                        signature: jit.order().signature.clone(),
                        receiver: Some(jit.order().receiver),
                        created: u32::try_from(Utc::now().timestamp())
                            .unwrap_or(u32::MIN)
                            .into(),
                        valid_to: jit.order().valid_to,
                        app_data: jit.order().app_data.into(),
                        partial: jit.order().partially_fillable(),
                        pre_interactions: vec![],
                        post_interactions: vec![],
                        sell_token_balance: jit.order().sell_token_balance,
                        buy_token_balance: jit.order().buy_token_balance,
                        protocol_fees: vec![],
                        quote: None,
                    },
                    jit.executed(),
                    Fee::Dynamic(jit.fee()),
                )
                .map_err(error::Solution::InvalidJitTrade)?,
            );
            tracing::debug!(
                fulfillment = ?trade,
                "converted surplus capturing JIT trade into fulfillment"
            );
        }

        let solution = Self {
            id,
            trades,
            prices,
            pre_interactions,
            interactions,
            post_interactions,
            solver,
            weth,
            gas,
            flashloans,
        };

        // Check that the solution includes clearing prices for all user trades.
        if solution.user_trades().any(|trade| {
            solution.clearing_price(trade.order().sell.token).is_none()
                || solution.clearing_price(trade.order().buy.token).is_none()
        }) {
            return Err(error::Solution::InvalidClearingPrices);
        }

        // Apply protocol fees only if the drivers is set to handler the fees
        if fee_handler != FeeHandler::Driver {
            return Ok(solution);
        }

        let mut trades = Vec::with_capacity(solution.trades.len());
        for trade in solution.trades {
            match &trade {
                Trade::Fulfillment(fulfillment) => {
                    let prices = ClearingPrices {
                        sell: solution.prices
                            [&fulfillment.order().sell.token.as_erc20(solution.weth)],
                        buy: solution.prices
                            [&fulfillment.order().buy.token.as_erc20(solution.weth)],
                    };
                    let fulfillment = fulfillment.with_protocol_fees(prices)?;
                    trades.push(Trade::Fulfillment(fulfillment))
                }
                Trade::Jit(_) => trades.push(trade),
            }
        }
        Ok(Self { trades, ..solution })
    }

    /// The ID of this solution.
    pub fn id(&self) -> &Id {
        &self.id
    }

    /// Trades settled by this solution.
    pub fn trades(&self) -> &[Trade] {
        &self.trades
    }

    /// Returns all the token pairs involved in the solution.
    pub fn token_pairs(&self) -> Vec<(TokenAddress, TokenAddress)> {
        self.trades
            .iter()
            .map(|trade| match trade {
                Trade::Fulfillment(fulfillment) => {
                    let order = fulfillment.order();
                    (order.sell.token, order.buy.token)
                }
                Trade::Jit(jit) => {
                    let order = jit.order();
                    (order.sell.token, order.buy.token)
                }
            })
            .collect()
    }

    /// Interactions executed by this solution.
    pub fn interactions(&self) -> &[Interaction] {
        &self.interactions
    }

    pub fn pre_interactions(&self) -> &[eth::Interaction] {
        &self.pre_interactions
    }

    /// The solver which generated this solution.
    pub fn solver(&self) -> &Solver {
        &self.solver
    }

    pub fn gas(&self) -> Option<eth::Gas> {
        self.gas
    }

    fn trade_count_for_scorable(
        &self,
        trade: &Trade,
        surplus_capturing_jit_order_owners: &HashSet<eth::Address>,
    ) -> bool {
        match trade {
            Trade::Fulfillment(_) => true,
            Trade::Jit(jit) => {
                surplus_capturing_jit_order_owners.contains(&jit.order().signature.signer)
            }
        }
    }

    /// JIT score calculation as per CIP38
    pub fn scoring(
        &self,
        prices: &auction::Prices,
        surplus_capturing_jit_order_owners: &HashSet<eth::Address>,
    ) -> Result<eth::Ether, error::Scoring> {
        let mut trades = Vec::with_capacity(self.trades.len());
        for trade in self.trades().iter().filter(|trade| {
            self.trade_count_for_scorable(trade, surplus_capturing_jit_order_owners)
        }) {
            // Solver generated fulfillment does not include the fee in the executed amount
            // for sell orders.
            let executed = match trade.side() {
                order::Side::Sell => (trade.executed().0 + trade.fee().0).into(),
                order::Side::Buy => trade.executed(),
            };
            let buy = trade.buy();
            let sell = trade.sell();
            let uniform_prices = ClearingPrices {
                sell: self
                    .clearing_price(sell.token)
                    .ok_or(error::Scoring::InvalidClearingPrices)?,
                buy: self
                    .clearing_price(buy.token)
                    .ok_or(error::Scoring::InvalidClearingPrices)?,
            };
            trades.push(scoring::Trade::new(
                sell,
                buy,
                trade.side(),
                executed,
                trade.custom_prices(&uniform_prices)?,
                trade.protocol_fees(),
            ))
        }

        let scoring = scoring::Scoring::new(trades);
        scoring.score(prices).map_err(error::Scoring::from)
    }

    /// Approval interactions necessary for encoding the settlement.
    pub async fn approvals(
        &self,
        eth: &Ethereum,
        internalization: settlement::Internalization,
    ) -> Result<impl Iterator<Item = eth::allowance::Approval>, Error> {
        let settlement_contract = &eth.contracts().settlement();
        let allowances =
            try_join_all(self.allowances(internalization).map(|required| async move {
                eth.erc20(required.0.token)
                    .allowance(settlement_contract.address().into(), required.0.spender)
                    .await
                    .map(|existing| (required, existing))
            }))
            .await?;
        let approvals = allowances
            .into_iter()
            .filter_map(|(required, existing)| required.approval(&existing));
        Ok(approvals)
    }

    /// An empty solution has no trades which is allowed to capture surplus and
    /// a score of 0.
    pub fn is_empty(&self, surplus_capturing_jit_order_owners: &HashSet<eth::Address>) -> bool {
        !self
            .trades
            .iter()
            .any(|trade| self.trade_count_for_scorable(trade, surplus_capturing_jit_order_owners))
    }

    pub fn merge(&self, other: &Self) -> Result<Self, error::Merge> {
        // We can only merge solutions from the same solver
        if self.solver.account().address() != other.solver.account().address() {
            return Err(error::Merge::Incompatible("Solvers"));
        }

        // Solutions should not settle the same order twice
        let uids: HashSet<_> = self.user_trades().map(|t| t.order().uid).collect();
        let other_uids: HashSet<_> = other.user_trades().map(|t| t.order().uid).collect();
        if !uids.is_disjoint(&other_uids) {
            return Err(error::Merge::DuplicateTrade);
        }

        // Solution prices need to be congruent, i.e. there needs to be a unique factor
        // to scale all common tokens from one solution into the other.
        let factor =
            scaling_factor(&self.prices, &other.prices).ok_or(error::Merge::IncongruentPrices)?;

        // To avoid precision issues, make sure we always scale up settlements
        if factor < BigRational::one() {
            return other.merge(self);
        }

        // Scale prices
        let mut prices = self.prices.clone();
        for (token, price) in other.prices.iter() {
            let scaled = number::conversions::big_rational_to_u256(
                &(number::conversions::u256_to_big_rational(price) * &factor),
            )
            .map_err(error::Merge::Math)?;
            match prices.entry(*token) {
                Entry::Occupied(entry) => {
                    // This shouldn't fail unless there are rounding errors given that the scaling
                    // factor is unique
                    if *entry.get() != scaled {
                        return Err(error::Merge::IncongruentPrices);
                    }
                }
                Entry::Vacant(entry) => {
                    entry.insert(scaled);
                }
            }
        }

        // Merge remaining fields
        Ok(Solution {
            id: Id::new_merged(&self.id, &other.id),
            trades: [self.trades.clone(), other.trades.clone()].concat(),
            prices,
            pre_interactions: [
                self.pre_interactions.clone(),
                other.pre_interactions.clone(),
            ]
            .concat(),
            interactions: [self.interactions.clone(), other.interactions.clone()].concat(),
            post_interactions: [
                self.post_interactions.clone(),
                other.post_interactions.clone(),
            ]
            .concat(),
            solver: self.solver.clone(),
            weth: self.weth,
            // Same solver are guaranteed to have the same fee handler
            gas: match (self.gas, other.gas) {
                (Some(gas), Some(other_gas)) => Some(gas + other_gas),
                (Some(gas), None) => Some(gas),
                (None, Some(gas)) => Some(gas),
                (None, None) => None,
            },
            flashloans: [self.flashloans.clone(), other.flashloans.clone()].concat(),
        })
    }

    /// Return the trades which fulfill non-liquidity auction orders. These are
    /// the orders placed by end users.
    fn user_trades(&self) -> impl Iterator<Item = &trade::Fulfillment> {
        self.trades.iter().filter_map(|trade| match trade {
            Trade::Fulfillment(fulfillment) => Some(fulfillment),
            Trade::Jit(_) => None,
        })
    }

    /// Return the allowances in a normalized form, where there is only one
    /// allowance per [`eth::allowance::Spender`], and they're ordered
    /// deterministically.
    fn allowances(
        &self,
        internalization: settlement::Internalization,
    ) -> impl Iterator<Item = eth::allowance::Required> {
        let mut normalized = HashMap::new();
        let allowances = self.interactions.iter().flat_map(|interaction| {
            if interaction.internalize()
                && matches!(internalization, settlement::Internalization::Enable)
            {
                vec![]
            } else {
                interaction.allowances()
            }
        });
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
        solver_native_token: ManageNativeToken,
    ) -> Result<Settlement, Error> {
        Settlement::encode(self, auction, eth, simulator, solver_native_token).await
    }

    /// Token prices settled by this solution, expressed using an arbitrary
    /// reference unit chosen by the solver. These values are only
    /// meaningful in relation to each others.
    ///
    /// The rule which relates two prices for tokens X and Y is:
    /// amount_x * price_x = amount_y * price_y
    pub fn clearing_prices(&self) -> Prices {
        let prices = self.prices.clone();

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
            let mut prices: Prices = if self.user_trades().all(|trade| {
                trade.order().sell.token != self.weth.0 && trade.order().buy.token != self.weth.0
            }) {
                prices
                    .into_iter()
                    .filter(|(token, _price)| *token != self.weth.0)
                    .collect()
            } else {
                prices
            };

            // Add a clearing price for ETH equal to WETH.
            prices.insert(eth::ETH_TOKEN, self.prices[&self.weth.into()].to_owned());

            return prices;
        }

        // TODO: We should probably filter out all unused prices to save gas.

        prices
    }

    /// Clearing price for the given token.
    pub fn clearing_price(&self, token: eth::TokenAddress) -> Option<eth::U256> {
        // The clearing price of ETH is equal to WETH.
        let token = token.as_erc20(self.weth);
        self.prices.get(&token).map(ToOwned::to_owned)
    }

    /// Whether there is a reasonable risk of this solution reverting on chain.
    pub fn revertable(&self) -> bool {
        self.interactions
            .iter()
            .any(|interaction| !interaction.internalize())
            || self.user_trades().any(|trade| {
                matches!(
                    trade.order().signature.scheme,
                    order::signature::Scheme::Eip1271
                )
            })
    }
}

impl std::fmt::Debug for Solution {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        f.debug_struct("Solution")
            .field("id", &self.id)
            .field("trades", &self.trades)
            .field("prices", &self.prices)
            .field("pre_interactions", &self.pre_interactions)
            .field("interactions", &self.interactions)
            .field("post_interactions", &self.post_interactions)
            .field("solver", &self.solver.name())
            .finish()
    }
}

/// Given two solutions returns the factors with
/// which prices of the second solution would have to be multiplied so that the
/// given token would have the same price in both solutions.
/// If the solutions have no prices in common any scaling factor is valid (we
/// return 1). Returns None if the solutions have more than one price in common
/// and the scaling factor is not unique.
fn scaling_factor(first: &Prices, second: &Prices) -> Option<BigRational> {
    let factors: HashSet<_> = first
        .keys()
        .collect::<HashSet<_>>()
        .intersection(&second.keys().collect::<HashSet<_>>())
        .map(|&token| {
            let first_price = first[token];
            let second_price = second[token];
            BigRational::new(
                number::conversions::u256_to_big_int(&first_price),
                number::conversions::u256_to_big_int(&second_price),
            )
        })
        .collect();
    match factors.len() {
        0 => Some(BigRational::one()),
        1 => factors.into_iter().next(),
        _ => None,
    }
}

/// A unique reference to a specific solution which consists of 2 parts:
/// 1. Globally unique (until driver restarts) ID used to communicate with the
///    protocol. Global uniquenes is enforced by the constructors.
/// 2. List of merged sub ids. Each sub id was generated by the solver and only
///    has to be unique within an auction run loop. If this list contains only a
///    single id it means this Id belongs to an unmodified solution provided as
///    is by the solver. If it contains multiple sub ids multiple base solutions
///    have been merged into a bigger one.
#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub struct Id {
    id: u64,
    merged_solutions: Vec<u64>,
}

impl Id {
    fn next_global_id() -> u64 {
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        COUNTER.fetch_add(1, Ordering::Relaxed)
    }

    pub fn new(solution: u64) -> Self {
        Self {
            id: Self::next_global_id(),
            merged_solutions: vec![solution],
        }
    }

    pub fn new_merged(first: &Self, second: &Self) -> Self {
        let merged_solutions = first
            .solutions()
            .iter()
            .chain(second.solutions().iter())
            .copied()
            .collect();
        Self {
            id: Self::next_global_id(),
            merged_solutions,
        }
    }

    /// Globally unique id communicated to the protocol.
    pub fn get(&self) -> u64 {
        self.id
    }

    /// Which base solutions have been merged into this complete solution.
    /// Ids in this list are only unique within one auction run loop.
    pub fn solutions(&self) -> &[u64] {
        &self.merged_solutions
    }
}

pub mod error {
    use super::*;

    #[derive(Debug, thiserror::Error)]
    pub enum Merge {
        #[error("incompatible {0:?}")]
        Incompatible(&'static str),
        #[error("duplicate trade")]
        DuplicateTrade,
        #[error("incongruent prices")]
        IncongruentPrices,
        #[error("math error: {0:?}")]
        Math(anyhow::Error),
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
            "non bufferable tokens used: solution attempts to internalize tokens which are not \
             trusted"
        )]
        NonBufferableTokensUsed(BTreeSet<TokenAddress>),
        #[error("invalid internalization: uninternalized solution fails to simulate")]
        FailingInternalization,
        #[error("Gas estimate of {0:?} exceeded the per settlement limit of {1:?}")]
        GasLimitExceeded(eth::Gas, eth::Gas),
        #[error("insufficient solver account Ether balance, required {0:?}")]
        SolverAccountInsufficientBalance(eth::Ether),
        #[error("attempted to merge settlements generated by different solvers")]
        DifferentSolvers,
        #[error("encoding error: {0:?}")]
        Encoding(#[from] encoding::Error),
    }

    #[derive(Debug, thiserror::Error)]
    pub enum Math {
        #[error("overflow")]
        Overflow,
        #[error("division by zero")]
        DivisionByZero,
        #[error("negative")]
        Negative,
    }

    #[derive(Debug, thiserror::Error)]
    pub enum Solution {
        #[error("invalid clearing prices")]
        InvalidClearingPrices,
        #[error(transparent)]
        ProtocolFee(#[from] fee::Error),
        #[error("invalid JIT trade")]
        InvalidJitTrade(Trade),
    }

    #[derive(Debug, thiserror::Error)]
    pub enum Scoring {
        #[error("invalid clearing prices")]
        InvalidClearingPrices,
        #[error(transparent)]
        Math(#[from] Math),
        #[error("failed to calculate custom prices")]
        CalculateCustomPrices(#[source] Trade),
        #[error("missing native price for token {0:?}")]
        MissingPrice(TokenAddress),
    }

    impl From<scoring::Error> for Scoring {
        fn from(value: scoring::Error) -> Self {
            match value {
                scoring::Error::MissingPrice(e) => Self::MissingPrice(e),
                scoring::Error::Math(e) => Self::Math(e),
                scoring::Error::Scoring(e) => e,
            }
        }
    }

    #[derive(Debug, thiserror::Error)]
    pub enum Trade {
        #[error("orders with non solver determined gas cost fees are not supported")]
        ProtocolFeeOnStaticOrder,
        #[error("invalid executed amount")]
        InvalidExecutedAmount,
        #[error(transparent)]
        Math(#[from] Math),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Tests that constructor ensures unique ids.
    #[test]
    fn solution_id_unique() {
        let first = Id::new(111);
        assert_eq!(first.get(), 0);
        assert_eq!(first.solutions(), &[111]);

        let second = Id::new(222);
        assert_eq!(second.get(), 1);
        assert_eq!(second.solutions(), &[222]);

        let third = Id::new_merged(&first, &second);
        assert_eq!(third.get(), 2);
        assert_eq!(third.solutions(), &[111, 222]);

        let fourth = Id::new_merged(&second, &first);
        assert_eq!(fourth.get(), 3);
        assert_eq!(fourth.solutions(), &[222, 111]);
    }
}
