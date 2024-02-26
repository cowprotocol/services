use {
    super::{Estimate, Verification},
    crate::{
        code_fetching::CodeFetching,
        code_simulation::CodeSimulating,
        encoded_settlement::{encode_trade, EncodedSettlement},
        interaction::EncodedInteraction,
        trade_finding::{Interaction, Trade},
    },
    anyhow::{Context, Result},
    contracts::{
        deployed_bytecode,
        dummy_contract,
        support::{Solver, Trader},
        GPv2Settlement,
        WETH9,
    },
    ethcontract::{tokens::Tokenize, Bytes, H160, U256},
    ethrpc::{current_block::CurrentBlockStream, extensions::StateOverride},
    maplit::hashmap,
    model::{
        order::{OrderData, OrderKind, BUY_ETH_ADDRESS},
        signature::{Signature, SigningScheme},
    },
    number::nonzero::U256 as NonZeroU256,
    std::sync::Arc,
    web3::{ethabi::Token, types::CallRequest},
};

#[async_trait::async_trait]
pub trait TradeVerifying: Send + Sync + 'static {
    /// Verifies that the proposed [`Trade`] actually fulfills the
    /// [`PriceQuery`] and returns a price [`Estimate`] that is trustworthy.
    async fn verify(
        &self,
        query: &PriceQuery,
        verification: &Verification,
        trade: Trade,
    ) -> Result<VerifiedEstimate>;
}

#[derive(Copy, Clone, Debug, Default, Eq, PartialEq)]
pub struct VerifiedEstimate {
    pub out_amount: U256,
    pub gas: u64,
    pub solver: H160,
}

impl From<VerifiedEstimate> for Estimate {
    fn from(estimate: VerifiedEstimate) -> Self {
        Self {
            out_amount: estimate.out_amount,
            gas: estimate.gas,
            solver: estimate.solver,
            verified: true,
        }
    }
}

/// Component that verifies a trade is actually executable by simulating it
/// and determines a price estimate based off of that simulation.
#[derive(Clone)]
pub struct TradeVerifier {
    simulator: Arc<dyn CodeSimulating>,
    code_fetcher: Arc<dyn CodeFetching>,
    block_stream: CurrentBlockStream,
    settlement: H160,
    native_token: H160,
}

impl TradeVerifier {
    const DEFAULT_GAS: u64 = 8_000_000;
    const TRADER_IMPL: H160 = addr!("0000000000000000000000000000000000010000");

    pub fn new(
        simulator: Arc<dyn CodeSimulating>,
        code_fetcher: Arc<dyn CodeFetching>,
        block_stream: CurrentBlockStream,
        settlement: H160,
        native_token: H160,
    ) -> Self {
        Self {
            simulator,
            code_fetcher,
            block_stream,
            settlement,
            native_token,
        }
    }
}

#[async_trait::async_trait]
impl TradeVerifying for TradeVerifier {
    async fn verify(
        &self,
        query: &PriceQuery,
        verification: &Verification,
        trade: Trade,
    ) -> Result<VerifiedEstimate> {
        let start = std::time::Instant::now();
        let solver = dummy_contract!(Solver, trade.solver);

        let settlement = encode_settlement(query, verification, &trade, self.native_token);
        let settlement =
            add_balance_queries(settlement, query, verification, self.settlement, &solver);

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
            OrderKind::Sell => query.in_amount.get(),
            OrderKind::Buy => trade.out_amount,
        };

        let simulation = solver
            .methods()
            .swap(
                self.settlement,
                verification.from,
                query.sell_token,
                sell_amount,
                self.native_token,
                verification.receiver,
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
            verification.from => StateOverride {
                code: Some(deployed_bytecode!(Trader)),
                ..Default::default()
            },
            solver.address() => StateOverride {
                code: Some(deployed_bytecode!(Solver)),
                ..Default::default()
            },
        };

        let trader_impl = self
            .code_fetcher
            .code(verification.from)
            .await
            .context("failed to fetch trader code")?;
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

        let block = self.block_stream.borrow().number;
        let output = self
            .simulator
            .simulate(call, overrides, Some(block))
            .await
            .context("failed to simulate quote")?;
        let summary =
            SettleOutput::decode(&output).context("could not decode simulation output")?;
        let verified = VerifiedEstimate {
            out_amount: summary
                .out_amount(query.kind)
                .context("could not compute out_amount")?,
            gas: summary.gas_used.as_u64(),
            solver: trade.solver,
        };
        tracing::debug!(
            out_diff = ?trade.out_amount.abs_diff(verified.out_amount),
            gas_diff = ?trade.gas_estimate.abs_diff(verified.gas),
            time = ?start.elapsed(),
            promised_out_amount = ?trade.out_amount,
            promised_gas = trade.gas_estimate,
            ?verified,
            ?query,
            ?verification,
            "verified quote",
        );
        Ok(verified)
    }
}

fn encode_interactions(interactions: &[Interaction]) -> Vec<EncodedInteraction> {
    interactions.iter().map(|i| i.encode()).collect()
}

fn encode_settlement(
    query: &PriceQuery,
    verification: &Verification,
    trade: &Trade,
    native_token: H160,
) -> EncodedSettlement {
    let mut trade_interactions = encode_interactions(&trade.interactions);
    if query.buy_token == BUY_ETH_ADDRESS {
        // Because the `driver` manages `WETH` unwraps under the hood the `TradeFinder`
        // does not have to emit unwraps to pay out `ETH` in a trade.
        // However, for the simulation to be successful this has to happen so we do it
        // ourselves here.
        let buy_amount = match query.kind {
            OrderKind::Sell => trade.out_amount,
            OrderKind::Buy => query.in_amount.get(),
        };
        let weth = dummy_contract!(WETH9, native_token);
        let calldata = weth.methods().withdraw(buy_amount).tx.data.unwrap().0;
        trade_interactions.push((native_token, 0.into(), Bytes(calldata)));
        tracing::trace!("adding unwrap interaction for paying out ETH");
    }

    let tokens = vec![query.sell_token, query.buy_token];
    let clearing_prices = match query.kind {
        OrderKind::Sell => vec![trade.out_amount, query.in_amount.get()],
        OrderKind::Buy => vec![query.in_amount.get(), trade.out_amount],
    };

    // Configure the most disadvantageous trade possible (while taking possible
    // overflows into account). Should the trader not receive the amount promised by
    // the [`Trade`] the simulation will still work and we can compute the actual
    // [`Trade::out_amount`] afterwards.
    let (sell_amount, buy_amount) = match query.kind {
        OrderKind::Sell => (query.in_amount.get(), 0.into()),
        OrderKind::Buy => (
            trade.out_amount.max(U256::from(u128::MAX)),
            query.in_amount.get(),
        ),
    };
    let fake_order = OrderData {
        sell_token: query.sell_token,
        sell_amount,
        buy_token: query.buy_token,
        buy_amount,
        receiver: Some(verification.receiver),
        valid_to: u32::MAX,
        app_data: Default::default(),
        fee_amount: 0.into(),
        kind: query.kind,
        partially_fillable: false,
        sell_token_balance: verification.sell_token_source,
        buy_token_balance: verification.buy_token_destination,
    };

    let fake_signature = Signature::default_with(SigningScheme::Eip1271);
    let encoded_trade = encode_trade(
        &fake_order,
        &fake_signature,
        verification.from,
        0,
        1,
        &query.in_amount.get(),
    );

    EncodedSettlement {
        tokens,
        clearing_prices,
        trades: vec![encoded_trade],
        interactions: [
            encode_interactions(&verification.pre_interactions),
            trade_interactions,
            encode_interactions(&verification.post_interactions),
        ],
    }
}

/// Adds the interactions that are only needed to query important balances
/// throughout the simulation.
/// These balances will get used to compute an accurate price for the trade.
fn add_balance_queries(
    mut settlement: EncodedSettlement,
    query: &PriceQuery,
    verification: &Verification,
    settlement_contract: H160,
    solver: &Solver,
) -> EncodedSettlement {
    let (token, owner) = match query.kind {
        // track how much `buy_token` the `receiver` actually got
        OrderKind::Sell => (query.buy_token, verification.receiver),
        // track how much `sell_token` the settlement contract actually spent
        OrderKind::Buy => (query.sell_token, settlement_contract),
    };
    let query_balance = solver.methods().store_balance(token, owner);
    let query_balance = Bytes(query_balance.tx.data.unwrap().0);
    let interaction = (solver.address(), 0.into(), query_balance);
    // query balance right after we receive all `sell_token`
    settlement.interactions[1].insert(0, interaction.clone());
    // query balance right after we payed out all `buy_token`
    settlement.interactions[2].insert(0, interaction);
    settlement
}

/// Output of `Trader::settle` smart contract call.
#[derive(Debug)]
struct SettleOutput {
    /// Gas used for the `settle()` call.
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
        let balance_before = balances.first().context("no balance before settlement")?;
        let balance_after = balances.get(1).context("no balance after settlement")?;
        let out_amount = match kind {
            // for sell orders we track the buy_token amount which increases during the settlement
            OrderKind::Sell => balance_after.checked_sub(*balance_before),
            // for buy orders we track the sell_token amount which decreases during the settlement
            OrderKind::Buy => balance_before.checked_sub(*balance_after),
        };
        out_amount.context("underflow during out_amount computation")
    }
}

#[derive(Debug)]
pub struct PriceQuery {
    pub sell_token: H160,
    // This should be `BUY_ETH_ADDRESS` if you actually want to trade `ETH`
    pub buy_token: H160,
    pub kind: OrderKind,
    pub in_amount: NonZeroU256,
}
