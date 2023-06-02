//! Module with shared logic for creating a `PriceEstimating` implementation
//! from an inner `TradeFinding`.

use {
    super::{
        rate_limited,
        Estimate,
        PriceEstimateResult,
        PriceEstimating,
        PriceEstimationError,
        Query,
    },
    crate::{
        code_fetching::CodeFetching,
        code_simulation::CodeSimulating,
        encoded_settlement::{encode_trade, EncodedSettlement},
        ethrpc::extensions::StateOverride,
        interaction::EncodedInteraction,
        rate_limiter::RateLimiter,
        request_sharing::RequestSharing,
        trade_finding::{Interaction, Trade, TradeError, TradeFinding},
    },
    anyhow::{Context, Result},
    contracts::{
        support::{Solver, Trader},
        GPv2Settlement,
        WETH9,
    },
    ethcontract::{tokens::Tokenize, Bytes, H160, U256},
    futures::{
        future::{BoxFuture, FutureExt as _},
        stream::StreamExt as _,
    },
    maplit::hashmap,
    model::{
        order::{BuyTokenDestination, OrderData, OrderKind, SellTokenSource, BUY_ETH_ADDRESS},
        signature::{Signature, SigningScheme},
    },
    std::sync::Arc,
    web3::{ethabi::Token, types::CallRequest},
};

/// A `TradeFinding`-based price estimator with request sharing and rate
/// limiting.
pub struct TradeEstimator {
    inner: Arc<Inner>,
    sharing: RequestSharing<Query, BoxFuture<'static, Result<Estimate, PriceEstimationError>>>,
    rate_limiter: Arc<RateLimiter>,
}

#[derive(Clone)]
struct Inner {
    finder: Arc<dyn TradeFinding>,
    verifier: Option<TradeVerifier>,
}

/// A trade verifier.
#[derive(Clone)]
pub struct TradeVerifier {
    simulator: Arc<dyn CodeSimulating>,
    code_fetcher: Arc<dyn CodeFetching>,
    settlement: H160,
    native_token: H160,
}

impl TradeEstimator {
    pub fn new(finder: Arc<dyn TradeFinding>, rate_limiter: Arc<RateLimiter>) -> Self {
        Self {
            inner: Arc::new(Inner {
                finder,
                verifier: None,
            }),
            sharing: Default::default(),
            rate_limiter,
        }
    }

    pub fn with_verifier(mut self, verifier: TradeVerifier) -> Self {
        self.inner = Arc::new(Inner {
            verifier: Some(verifier),
            ..arc_unwrap_or_clone(self.inner)
        });
        self
    }

    async fn estimate(&self, query: Query) -> Result<Estimate, PriceEstimationError> {
        let Some(owner) = query.from else {
            return Err(PriceEstimationError::Other(anyhow::anyhow!("verified quotes require a 'from' and 'receiver' address")));
        };

        let quote_query = QuoteQuery {
            sell_token: query.sell_token,
            buy_token: query.buy_token,
            kind: query.kind,
            in_amount: query.in_amount,
            // TODO drop this trait implementation when we actually split quotes from price
            // estimates because trade verificaton requires data that doesn't exist for
            // price estimation requests.
            // Until then we try to keep this implementation as usable as posible.
            from: owner,
            receiver: owner,
            pre_interactions: vec![],
            post_interactions: vec![],
            sell_token_source: Default::default(),
            buy_token_destination: Default::default(),
        };
        let estimate = rate_limited(
            self.rate_limiter.clone(),
            self.inner.clone().estimate(quote_query),
        );
        self.sharing.shared(query, estimate.boxed()).await
    }
}

impl Inner {
    async fn estimate(
        self: Arc<Self>,
        query: QuoteQuery,
    ) -> Result<Estimate, PriceEstimationError> {
        let finder_query = Query {
            from: Some(query.from),
            sell_token: query.sell_token,
            buy_token: query.buy_token,
            kind: query.kind,
            in_amount: query.in_amount,
        };

        match &self.verifier {
            Some(verifier) => {
                let trade = self.finder.get_trade(&finder_query).await?;
                verifier
                    .verify(query, trade)
                    .await
                    .map_err(PriceEstimationError::Other)
            }
            None => {
                let quote = self.finder.get_quote(&finder_query).await?;
                Ok(Estimate {
                    out_amount: quote.out_amount,
                    gas: quote.gas_estimate,
                })
            }
        }
    }
}

fn encode_interactions(interactions: &[Interaction]) -> Vec<EncodedInteraction> {
    interactions.iter().map(|i| i.encode()).collect()
}

fn encode_settlement(query: &QuoteQuery, trade: &Trade, native_token: H160) -> EncodedSettlement {
    let mut trade_interactions = encode_interactions(&trade.interactions);
    if query.buy_token == BUY_ETH_ADDRESS {
        // Because the `driver` manages `WETH` unwraps under the hood the `TradeFinder`
        // does not have to emit unwraps to pay out `ETH` in a trade.
        // However, for the simulation to be successful this has to happen so we do it
        // ourselves here.
        let buy_amount = match query.kind {
            OrderKind::Sell => trade.out_amount,
            OrderKind::Buy => query.in_amount,
        };
        let weth = dummy_contract!(WETH9, native_token);
        let calldata = weth.methods().withdraw(buy_amount).tx.data.unwrap().0;
        trade_interactions.push((native_token, 0.into(), Bytes(calldata)));
        tracing::trace!("adding unwrap interaction for paying out ETH");
    }

    let tokens = vec![query.sell_token, query.buy_token];
    let clearing_prices = match query.kind {
        OrderKind::Sell => vec![trade.out_amount, query.in_amount],
        OrderKind::Buy => vec![query.in_amount, trade.out_amount],
    };

    // Configure the most disadvantageous trade possible. Should the trader not
    // receive the amount promised by the [`Trade`] the simulation will still work
    // and we can compute the actual [`Trade::out_amount`] afterwards.
    let (sell_amount, buy_amount) = match query.kind {
        OrderKind::Sell => (query.in_amount, 0.into()),
        OrderKind::Buy => (U256::MAX, query.in_amount),
    };
    let fake_order = OrderData {
        sell_token: query.sell_token,
        sell_amount,
        buy_token: query.buy_token,
        buy_amount,
        receiver: Some(query.receiver),
        valid_to: u32::MAX,
        app_data: Default::default(),
        fee_amount: 0.into(),
        kind: query.kind,
        partially_fillable: false,
        sell_token_balance: query.sell_token_source,
        buy_token_balance: query.buy_token_destination,
    };

    let fake_signature = Signature::default_with(SigningScheme::Eip1271);
    let encoded_trade = encode_trade(
        &fake_order,
        &fake_signature,
        query.from,
        0,
        1,
        &query.in_amount,
    );

    EncodedSettlement {
        tokens,
        clearing_prices,
        trades: vec![encoded_trade],
        interactions: [
            encode_interactions(&query.pre_interactions),
            trade_interactions,
            encode_interactions(&query.post_interactions),
        ],
    }
}

/// Adds the interactions that are only needed to query important balances
/// throughout the simulation.
/// These balances will get used to compute an accurate price for the trade.
fn add_balance_queries(
    mut settlement: EncodedSettlement,
    query: &QuoteQuery,
    settlement_contract: H160,
    solver: &Solver,
) -> EncodedSettlement {
    let (token, owner) = match query.kind {
        // track how much `buy_token` the `receiver` actually got
        OrderKind::Sell => (query.buy_token, query.receiver),
        // track how much `sell_token` the settlement contract actually spent
        OrderKind::Buy => (query.sell_token, settlement_contract),
    };
    let query_balance = solver.methods().store_balance_metered(token, owner);
    let query_balance = Bytes(query_balance.tx.data.unwrap().0);
    let interaction = (solver.address(), 0.into(), query_balance);
    settlement.interactions[1].insert(0, interaction.clone());
    match query.kind {
        // query `receiver` balance right after paying out funds (first post-interaction)
        OrderKind::Sell => settlement.interactions[2].insert(0, interaction),
        // query `settlement` balance right before paying out funds (last regular interaction)
        OrderKind::Buy => settlement.interactions[1].push(interaction),
    }
    settlement
}

impl TradeVerifier {
    const DEFAULT_GAS: u64 = 8_000_000;
    const TRADER_IMPL: H160 = addr!("0000000000000000000000000000000000010000");

    pub fn new(
        simulator: Arc<dyn CodeSimulating>,
        code_fetcher: Arc<dyn CodeFetching>,
        settlement: H160,
        native_token: H160,
    ) -> Self {
        Self {
            simulator,
            code_fetcher,
            settlement,
            native_token,
        }
    }

    async fn verify(&self, query: QuoteQuery, trade: Trade) -> Result<Estimate> {
        tracing::error!(?query, "verifying quote");
        // TODO: add solver address to [`Trade`]; for now we simply use Quasilab's
        // address
        let solver = dummy_contract!(
            Solver,
            H160(hex_literal::hex!(
                "1e8D9a45175B2a4122F7827ce1eA3B08327b2ba0"
            ))
        );

        let settlement = encode_settlement(&query, &trade, self.native_token);
        let settlement = add_balance_queries(settlement, &query, self.settlement, &solver);

        let settlement_contract = dummy_contract!(GPv2Settlement, self.settlement);
        let settlement = settlement_contract
            .methods()
            .settle(
                settlement.tokens,
                settlement.clearing_prices,
                settlement.trades,
                settlement.interactions,
            )
            .tx;

        let sell_amount = match query.kind {
            OrderKind::Sell => query.in_amount,
            OrderKind::Buy => trade.out_amount,
        };

        let simulation = solver
            .methods()
            .swap(
                query.from,
                query.sell_token,
                sell_amount,
                self.native_token,
                query.receiver,
                Bytes(settlement.data.unwrap().0),
            )
            .tx;

        let call = CallRequest {
            // Initiate tx as solver so gas doesn't get deducted from user's ETH.
            from: Some(solver.address()),
            to: Some(solver.address()),
            data: simulation.data,
            gas: Some(Self::DEFAULT_GAS.into()),
            ..Default::default()
        };

        // Set up helper contracts impersonating trader and solver.
        let mut overrides = hashmap! {
            query.from => StateOverride {
                code: Some(deployed_bytecode!(Trader)),
                ..Default::default()
            },
            solver.address() => StateOverride {
                code: Some(deployed_bytecode!(Solver)),
                ..Default::default()
            },
        };

        let trader_impl = self.code_fetcher.code(query.from).await?;
        if !trader_impl.0.is_empty() {
            // Store `owner` implementation so `Trader` helper contract can proxy to it.
            overrides.insert(
                Self::TRADER_IMPL,
                StateOverride {
                    code: Some(trader_impl),
                    ..Default::default()
                },
            );
        }

        let output = self.simulator.simulate(call, overrides).await?;
        let summary = SettleOutput::decode(&output)?;
        let estimate = Estimate {
            out_amount: summary.out_amount(query.kind)?,
            gas: summary
                .gas_used()
                .context("couldn't compute gas for quote")?,
        };
        Ok(estimate)
    }
}

impl Clone for TradeEstimator {
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
            sharing: Default::default(),
            rate_limiter: self.rate_limiter.clone(),
        }
    }
}

impl PriceEstimating for TradeEstimator {
    fn estimates<'a>(
        &'a self,
        queries: &'a [Query],
    ) -> futures::stream::BoxStream<'_, (usize, PriceEstimateResult)> {
        debug_assert!(queries.iter().all(|query| {
            query.buy_token != model::order::BUY_ETH_ADDRESS
                && query.sell_token != model::order::BUY_ETH_ADDRESS
                && query.sell_token != query.buy_token
        }));

        futures::stream::iter(queries)
            .then(|query| self.estimate(*query))
            .enumerate()
            .boxed()
    }
}

impl From<TradeError> for PriceEstimationError {
    fn from(err: TradeError) -> Self {
        match err {
            TradeError::NoLiquidity => Self::NoLiquidity,
            TradeError::UnsupportedOrderType => Self::UnsupportedOrderType,
            TradeError::RateLimited => Self::RateLimited,
            TradeError::Other(err) => Self::Other(err),
        }
    }
}

#[derive(Clone, Debug, Hash, Eq, PartialEq)]
pub struct QuoteQuery {
    pub from: H160,
    pub receiver: H160,
    pub sell_token: H160,
    // This should be `BUY_ETH_ADDRESS` if you actually want to trade `ETH`
    pub buy_token: H160,
    pub kind: OrderKind,
    pub in_amount: U256,
    pub pre_interactions: Vec<Interaction>,
    pub post_interactions: Vec<Interaction>,
    pub sell_token_source: SellTokenSource,
    pub buy_token_destination: BuyTokenDestination,
}

/// Output of `Trader::settle` smart contract call.
#[derive(Debug)]
struct SettleOutput {
    /// Gas used for the `settle()` call. This still includes gas overhead from
    /// measuring gas usage of helper function so that still needs to be
    /// discounted for accurate results.
    gas_used: U256,
    /// Balances queried during the simulation in the order specified during the
    /// simulation set up.
    queried_balances: Vec<U256>,
}

impl SettleOutput {
    fn decode(output: &[u8]) -> Result<Self> {
        let function = Solver::raw_contract().abi.function("swap").unwrap();
        let tokens = function.decode_output(output).context("decode")?;
        let (gas_used, queried_balances) = Tokenize::from_token(Token::Tuple(tokens))?;
        Ok(Self {
            gas_used,
            queried_balances,
        })
    }

    /// Computes the actual [`Trade::out_amount`] based on the simulation.
    fn out_amount(&self, kind: OrderKind) -> Result<U256> {
        let balances = &self.queried_balances;
        let balance_before = balances.get(0).context("no balance before settlement")?;
        let balance_after = balances.get(1).context("no balance after settlement")?;
        let out_amount = match kind {
            // for sell orders we track the buy_token amount which increases during the settlement
            OrderKind::Sell => balance_after.checked_sub(*balance_before),
            // for buy orders we track the sell_token amount which decreases during the settlement
            OrderKind::Buy => balance_before.checked_sub(*balance_after),
        };
        out_amount.context("underflow during out_amount computation")
    }

    /// Returns the units of gas used accounting for overhead measuring gas.
    fn gas_used(&self) -> Option<u64> {
        // Additional gas used per function call that we measure the gas use for.
        const OVERHEAD_PER_MEASUREMENT: u64 = 23_861;
        // How often we incurred overhead for measuring gas usage of function calls that
        // are only needed for simulation purposes but are **not** required for
        // the actual trade.
        const MEASUREMENTS: u64 = 2;
        const TOTAL_OVERHEAD: u64 = OVERHEAD_PER_MEASUREMENT * MEASUREMENTS;

        let gas_used = self.gas_used.checked_sub(TOTAL_OVERHEAD.into())?;

        if gas_used > u64::MAX.into() {
            None
        } else {
            Some(gas_used.as_u64())
        }
    }
}

fn arc_unwrap_or_clone<T>(arc: Arc<T>) -> T
where
    T: Clone,
{
    Arc::try_unwrap(arc).unwrap_or_else(|arc| (*arc).clone())
}
