use {
    super::{Estimate, Verification},
    crate::{
        code_fetching::CodeFetching,
        code_simulation::CodeSimulating,
        encoded_settlement::{encode_trade, EncodedSettlement},
        interaction::EncodedInteraction,
        trade_finding::{external::dto, Interaction, TradeKind},
    },
    anyhow::{Context, Result},
    bigdecimal::Zero,
    contracts::{
        deployed_bytecode,
        dummy_contract,
        support::{AnyoneAuthenticator, Solver, Trader},
        GPv2Settlement,
        WETH9,
    },
    ethcontract::{tokens::Tokenize, Bytes, H160, U256},
    ethrpc::{block_stream::CurrentBlockWatcher, extensions::StateOverride, Web3},
    maplit::hashmap,
    model::{
        order::{OrderData, OrderKind, BUY_ETH_ADDRESS},
        signature::{Signature, SigningScheme},
        DomainSeparator,
    },
    num::BigRational,
    number::{conversions::u256_to_big_rational, nonzero::U256 as NonZeroU256},
    std::{collections::HashMap, sync::Arc},
    web3::{ethabi::Token, types::CallRequest},
};

#[async_trait::async_trait]
pub trait TradeVerifying: Send + Sync + 'static {
    /// Verifies if the proposed [`TradeKind`] actually fulfills the
    /// [`PriceQuery`].
    async fn verify(
        &self,
        query: &PriceQuery,
        verification: &Verification,
        trade: TradeKind,
    ) -> Result<Estimate>;
}

/// Component that verifies a trade is actually executable by simulating it
/// and determines a price estimate based off of that simulation.
#[derive(Clone)]
pub struct TradeVerifier {
    web3: Web3,
    simulator: Arc<dyn CodeSimulating>,
    code_fetcher: Arc<dyn CodeFetching>,
    block_stream: CurrentBlockWatcher,
    settlement: GPv2Settlement,
    native_token: H160,
    quote_inaccuracy_limit: BigRational,
    domain_separator: DomainSeparator,
}

impl TradeVerifier {
    const DEFAULT_GAS: u64 = 8_000_000;
    const TRADER_IMPL: H160 = addr!("0000000000000000000000000000000000010000");

    pub async fn new(
        web3: Web3,
        simulator: Arc<dyn CodeSimulating>,
        code_fetcher: Arc<dyn CodeFetching>,
        block_stream: CurrentBlockWatcher,
        settlement: H160,
        native_token: H160,
        quote_inaccuracy_limit: BigRational,
    ) -> Result<Self> {
        let settlement_contract = GPv2Settlement::at(&web3, settlement);
        let domain_separator =
            DomainSeparator(settlement_contract.domain_separator().call().await?.0);
        Ok(Self {
            simulator,
            code_fetcher,
            block_stream,
            settlement: settlement_contract,
            native_token,
            quote_inaccuracy_limit,
            web3,
            domain_separator,
        })
    }

    async fn verify_inner(
        &self,
        query: &PriceQuery,
        verification: &Verification,
        trade: &TradeKind,
        out_amount: &U256,
    ) -> Result<Estimate, Error> {
        if verification.from.is_zero() {
            // Don't waste time on common simulations which will always fail.
            return Err(anyhow::anyhow!("trader is zero address").into());
        }

        let start = std::time::Instant::now();

        // Use `tx_origin` if response indicates that a special address is needed for
        // the simulation to pass. Otherwise just use the solver address.
        let solver = trade.tx_origin().unwrap_or(trade.solver());
        let solver = dummy_contract!(Solver, solver);

        let settlement = encode_settlement(
            query,
            verification,
            trade,
            out_amount,
            self.native_token,
            &self.domain_separator,
        )?;

        let settlement = add_balance_queries(
            settlement,
            query,
            verification,
            self.settlement.address(),
            &solver,
        );

        let settlement = self
            .settlement
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
            OrderKind::Buy => *out_amount,
        };

        let simulation = solver
            .methods()
            .swap(
                self.settlement.address(),
                verification.from,
                query.sell_token,
                sell_amount,
                query.buy_token,
                self.native_token,
                verification.receiver,
                Bytes(settlement.data.unwrap().0),
                // only if the user did not provide pre-interactions is it safe
                // to set up the trade's pre-conditions on behalf of the user.
                // if the user provided pre-interactions it's reasonable to assume
                // that they will set up all the necessary details for the trade.
                verification.pre_interactions.is_empty(),
            )
            .tx;

        let block = *self.block_stream.borrow();

        let call = CallRequest {
            // Initiate tx as solver so gas doesn't get deducted from user's ETH.
            from: Some(solver.address()),
            to: Some(solver.address()),
            data: simulation.data,
            gas: Some(Self::DEFAULT_GAS.into()),
            gas_price: Some(block.gas_price),
            ..Default::default()
        };

        let overrides = self
            .prepare_state_overrides(verification, trade)
            .await
            .map_err(Error::SimulationFailed)?;

        let output = self
            .simulator
            .simulate(call, overrides, Some(block.number))
            .await
            .context("failed to simulate quote")
            .map_err(Error::SimulationFailed);

        // TODO remove when quoters stop signing zeroex RFQ orders for `tx.origin:
        // 0x0000` (#2693)
        if let Err(err) = &output {
            // Currently we know that if a trade requests to be simulated from `tx.origin:
            // 0x0000` it's because the solver signs zeroex RFQ orders which
            // require that origin. However, setting this `tx.origin` actually
            // results in invalid RFQ orders and until the solver signs orders
            // for a different `tx.origin` we need to pretend these
            // quotes actually simulated successfully to not lose these competitive quotes
            // when we enable quote verification in prod.
            if trade.tx_origin() == Some(H160::zero()) {
                let estimate = Estimate {
                    out_amount: *out_amount,
                    gas: trade.gas_estimate().context("no gas estimate")?,
                    solver: trade.solver(),
                    verified: true,
                };
                tracing::warn!(
                    ?estimate,
                    ?err,
                    "quote used invalid zeroex RFQ order; pass verification anyway"
                );
                return Ok(estimate);
            }
        };

        let mut summary = SettleOutput::decode(&output?, query.kind)
            .context("could not decode simulation output")
            .map_err(Error::SimulationFailed)?;

        {
            // Quote accuracy gets determined by how many tokens had to be paid out of the
            // settlement buffers to make the quote happen. When the settlement contract
            // itself is the trader or receiver these values need to be adjusted slightly.
            let (sell_amount, buy_amount) = match query.kind {
                OrderKind::Sell => (query.in_amount.get(), summary.out_amount),
                OrderKind::Buy => (summary.out_amount, query.in_amount.get()),
            };

            // It looks like the contract lost a lot of sell tokens but only because it was
            // the trader and had to pay for the trade. Adjust tokens lost downward.
            if verification.from == self.settlement.address() {
                summary.sell_tokens_lost -= u256_to_big_rational(&sell_amount);
            }
            // It looks like the contract gained a lot of buy tokens (negative loss) but
            // only because it was the receiver and got the payout. Adjust the tokens lost
            // upward.
            if verification.receiver == self.settlement.address() {
                summary.buy_tokens_lost += u256_to_big_rational(&buy_amount);
            }
        }

        tracing::debug!(
            lost_buy_amount = %summary.buy_tokens_lost,
            lost_sell_amount = %summary.sell_tokens_lost,
            gas_diff = ?trade.gas_estimate().unwrap_or_default().abs_diff(summary.gas_used.as_u64()),
            time = ?start.elapsed(),
            promised_out_amount = ?out_amount,
            verified_out_amount = ?summary.out_amount,
            promised_gas = trade.gas_estimate(),
            verified_gas = ?summary.gas_used,
            out_diff = ?out_amount.abs_diff(summary.out_amount),
            ?query,
            ?verification,
            "verified quote",
        );

        ensure_quote_accuracy(&self.quote_inaccuracy_limit, query, trade, &summary)
    }

    /// Configures all the state overrides that are needed to mock the given
    /// trade.
    async fn prepare_state_overrides(
        &self,
        verification: &Verification,
        trade: &TradeKind,
    ) -> Result<HashMap<H160, StateOverride>> {
        // Set up mocked trader.
        let mut overrides = hashmap! {
            verification.from => StateOverride {
                code: Some(deployed_bytecode!(Trader)),
                ..Default::default()
            },
        };

        // If the trader is a smart contract we also need to store its implementation
        // to proxy into it during the simulation.
        let trader_impl = self
            .code_fetcher
            .code(verification.from)
            .await
            .context("failed to fetch trader code")?;
        if !trader_impl.0.is_empty() {
            overrides.insert(
                Self::TRADER_IMPL,
                StateOverride {
                    code: Some(trader_impl),
                    ..Default::default()
                },
            );
        }

        // Set up mocked solver.
        let mut solver_override = StateOverride {
            code: Some(deployed_bytecode!(Solver)),
            ..Default::default()
        };

        // If the trade requires a special tx.origin we also need to fake the
        // authenticator and tx origin balance.
        if trade
            .tx_origin()
            .is_some_and(|origin| origin != trade.solver())
        {
            let (authenticator, balance) = futures::join!(
                self.settlement.authenticator().call(),
                self.web3.eth().balance(trade.solver(), None)
            );
            let authenticator = authenticator.context("could not fetch authenticator")?;
            overrides.insert(
                authenticator,
                StateOverride {
                    code: Some(deployed_bytecode!(AnyoneAuthenticator)),
                    ..Default::default()
                },
            );
            let balance = balance.context("could not fetch balance")?;
            solver_override.balance = Some(balance);
        }
        overrides.insert(trade.tx_origin().unwrap_or(trade.solver()), solver_override);

        Ok(overrides)
    }
}

#[async_trait::async_trait]
impl TradeVerifying for TradeVerifier {
    async fn verify(
        &self,
        query: &PriceQuery,
        verification: &Verification,
        trade: TradeKind,
    ) -> Result<Estimate> {
        let out_amount = trade
            .out_amount(
                &query.buy_token,
                &query.sell_token,
                &query.in_amount.get(),
                &query.kind,
            )
            .context("failed to compute trade out amount")?;
        match self
            .verify_inner(query, verification, &trade, &out_amount)
            .await
        {
            Ok(verified) => Ok(verified),
            Err(Error::SimulationFailed(err)) => match trade.gas_estimate() {
                Some(gas) => {
                    let estimate = Estimate {
                        out_amount,
                        gas,
                        solver: trade.solver(),
                        verified: false,
                    };
                    tracing::warn!(
                        ?err,
                        estimate = ?trade,
                        "failed verification; returning unverified estimate"
                    );
                    Ok(estimate)
                }
                None => {
                    tracing::warn!(
                        ?err,
                        estimate = ?trade,
                        "failed verification and no gas estimate provided; discarding estimate"
                    );
                    Err(err)
                }
            },
            Err(err @ Error::TooInaccurate) => {
                tracing::warn!("discarding quote because it's too inaccurate");
                Err(err.into())
            }
        }
    }
}

fn encode_interactions(interactions: &[Interaction]) -> Vec<EncodedInteraction> {
    interactions.iter().map(|i| i.encode()).collect()
}

fn encode_settlement(
    query: &PriceQuery,
    verification: &Verification,
    trade: &TradeKind,
    out_amount: &U256,
    native_token: H160,
    domain_separator: &DomainSeparator,
) -> Result<EncodedSettlement> {
    let mut trade_interactions = encode_interactions(&trade.interactions());
    if query.buy_token == BUY_ETH_ADDRESS {
        // Because the `driver` manages `WETH` unwraps under the hood the `TradeFinder`
        // does not have to emit unwraps to pay out `ETH` in a trade.
        // However, for the simulation to be successful this has to happen so we do it
        // ourselves here.
        let buy_amount = match query.kind {
            OrderKind::Sell => *out_amount,
            OrderKind::Buy => query.in_amount.get(),
        };
        let weth = dummy_contract!(WETH9, native_token);
        let calldata = weth.methods().withdraw(buy_amount).tx.data.unwrap().0;
        trade_interactions.push((native_token, 0.into(), Bytes(calldata)));
        tracing::trace!("adding unwrap interaction for paying out ETH");
    }

    let (tokens, clearing_prices) = match trade {
        TradeKind::Legacy(_) => {
            let tokens = vec![query.sell_token, query.buy_token];
            let prices = match query.kind {
                OrderKind::Sell => vec![*out_amount, query.in_amount.get()],
                OrderKind::Buy => vec![query.in_amount.get(), *out_amount],
            };
            (tokens, prices)
        }
        TradeKind::Regular(trade) => {
            let mut tokens = Vec::with_capacity(trade.clearing_prices.len());
            let mut prices = Vec::with_capacity(trade.clearing_prices.len());
            for (token, price) in trade.clearing_prices.iter() {
                tokens.push(*token);
                prices.push(*price);
            }
            (tokens, prices)
        }
    };

    // Configure the most disadvantageous trade possible (while taking possible
    // overflows into account). Should the trader not receive the amount promised by
    // the [`Trade`] the simulation will still work and we can compute the actual
    // [`Trade::out_amount`] afterwards.
    let (sell_amount, buy_amount) = match query.kind {
        OrderKind::Sell => (query.in_amount.get(), 0.into()),
        OrderKind::Buy => (
            (*out_amount).max(U256::from(u128::MAX)),
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
        // the tokens set length is small so the linear search is acceptable
        tokens
            .iter()
            .position(|token| token == &query.sell_token)
            .context("missing sell token index")?,
        tokens
            .iter()
            .position(|token| token == &query.buy_token)
            .context("missing buy token index")?,
        &query.in_amount.get(),
    );

    let mut trades = vec![encoded_trade];

    if let TradeKind::Regular(trade) = trade {
        for jit_order in trade.jit_orders.iter() {
            let order_data = OrderData {
                sell_token: jit_order.sell_token,
                buy_token: jit_order.buy_token,
                receiver: Some(jit_order.receiver),
                sell_amount: jit_order.sell_amount,
                buy_amount: jit_order.buy_amount,
                valid_to: jit_order.valid_to,
                app_data: jit_order.app_data,
                fee_amount: 0.into(),
                kind: match &jit_order.side {
                    dto::Side::Buy => OrderKind::Buy,
                    dto::Side::Sell => OrderKind::Sell,
                },
                partially_fillable: jit_order.partially_fillable,
                sell_token_balance: jit_order.sell_token_source,
                buy_token_balance: jit_order.buy_token_destination,
            };
            let (owner, signature) = match jit_order.signing_scheme {
                SigningScheme::Eip1271 => {
                    let (owner, signature) = jit_order.signature.split_at(20);
                    let owner = H160::from_slice(owner);
                    let signature = Signature::from_bytes(jit_order.signing_scheme, signature)?;
                    (owner, signature)
                }
                SigningScheme::PreSign => {
                    let owner = H160::from_slice(&jit_order.signature);
                    let signature =
                        Signature::from_bytes(jit_order.signing_scheme, Vec::new().as_slice())?;
                    (owner, signature)
                }
                _ => {
                    let signature =
                        Signature::from_bytes(jit_order.signing_scheme, &jit_order.signature)?;
                    let owner = signature
                        .recover(domain_separator, &order_data.hash_struct())?
                        .context("could not recover the owner")?
                        .signer;
                    (owner, signature)
                }
            };

            trades.push(encode_trade(
                &order_data,
                &signature,
                owner,
                // the tokens set length is small so the linear search is acceptable
                tokens
                    .iter()
                    .position(|token| token == &jit_order.sell_token)
                    .context("missing jit order sell token index")?,
                tokens
                    .iter()
                    .position(|token| token == &jit_order.buy_token)
                    .context("missing jit order buy token index")?,
                &jit_order.executed_amount,
            ));
        }
    }
    let mut pre_interactions = verification.pre_interactions.clone();
    pre_interactions.extend(trade.pre_interactions().iter().cloned());

    Ok(EncodedSettlement {
        tokens,
        clearing_prices,
        trades,
        interactions: [
            encode_interactions(&pre_interactions),
            trade_interactions,
            encode_interactions(&verification.post_interactions),
        ],
    })
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
        OrderKind::Sell => {
            let receiver = match verification.receiver == H160::zero() {
                // Settlement contract sends fund to owner if receiver is the 0 address.
                true => verification.from,
                false => verification.receiver,
            };

            (query.buy_token, receiver)
        }
        // track how much `sell_token` the settlement contract actually spent
        OrderKind::Buy => (query.sell_token, settlement_contract),
    };
    let query_balance = solver.methods().store_balance(token, owner, true);
    let query_balance = Bytes(query_balance.tx.data.unwrap().0);
    let interaction = (solver.address(), 0.into(), query_balance);
    // query balance right after we receive all `sell_token`
    settlement.interactions[1].insert(0, interaction.clone());
    // query balance right after we payed out all `buy_token`
    settlement.interactions[2].insert(0, interaction);
    settlement
}

/// Analyzed output of `Solver::settle` smart contract call.
#[derive(Debug)]
struct SettleOutput {
    /// Gas used for the `settle()` call.
    gas_used: U256,
    /// `out_amount` perceived by the trader (sell token for buy orders or buy
    /// token for sell order)
    out_amount: U256,
    /// Difference in buy tokens of the settlement contract before and after the
    /// trade.
    buy_tokens_lost: BigRational,
    /// Difference in sell tokens of the settlement contract before and after
    /// the trade.
    sell_tokens_lost: BigRational,
}

impl SettleOutput {
    fn decode(output: &[u8], kind: OrderKind) -> Result<Self> {
        let function = Solver::raw_contract()
            .interface
            .abi
            .function("swap")
            .unwrap();
        let tokens = function.decode_output(output).context("decode")?;
        let (gas_used, balances): (U256, Vec<U256>) = Tokenize::from_token(Token::Tuple(tokens))?;

        let settlement_sell_balance_before = u256_to_big_rational(&balances[0]);
        let settlement_buy_balance_before = u256_to_big_rational(&balances[1]);

        let trader_balance_before = balances[2];
        let trader_balance_after = balances[3];

        let settlement_sell_balance_after = u256_to_big_rational(&balances[4]);
        let settlement_buy_balance_after = u256_to_big_rational(&balances[5]);

        let out_amount = match kind {
            // for sell orders we track the buy_token amount which increases during the settlement
            OrderKind::Sell => trader_balance_after.checked_sub(trader_balance_before),
            // for buy orders we track the sell_token amount which decreases during the settlement
            OrderKind::Buy => trader_balance_before.checked_sub(trader_balance_after),
        };
        let out_amount = out_amount.context("underflow during out_amount computation")?;

        Ok(SettleOutput {
            gas_used,
            out_amount,
            buy_tokens_lost: settlement_buy_balance_before - settlement_buy_balance_after,
            sell_tokens_lost: settlement_sell_balance_before - settlement_sell_balance_after,
        })
    }
}

/// Returns an error if settling the quote would require using too much of the
/// settlement contract buffers.
fn ensure_quote_accuracy(
    inaccuracy_limit: &BigRational,
    query: &PriceQuery,
    trade: &TradeKind,
    summary: &SettleOutput,
) -> std::result::Result<Estimate, Error> {
    // amounts verified by the simulation
    let (sell_amount, buy_amount) = match query.kind {
        OrderKind::Buy => (summary.out_amount, query.in_amount.get()),
        OrderKind::Sell => (query.in_amount.get(), summary.out_amount),
    };
    let (sell_amount, buy_amount) = (
        u256_to_big_rational(&sell_amount),
        u256_to_big_rational(&buy_amount),
    );

    let (expected_sell_token_lost, expected_buy_token_lost) = match trade {
        TradeKind::Regular(trade) => {
            let jit_orders_net_token_changes = trade.jit_orders_net_token_changes()?;
            let jit_orders_sell_token_changes = jit_orders_net_token_changes
                .get(&query.sell_token)
                .context("missing jit orders sell token executed amount")?;
            let jit_orders_buy_token_changes =
                jit_orders_net_token_changes
                    .get(&query.buy_token)
                    .context("missing jit orders buy token executed amount")?;

            let expected_sell_token_lost = -&sell_amount - jit_orders_sell_token_changes;
            let expected_buy_token_lost = &buy_amount - jit_orders_buy_token_changes;
            (expected_sell_token_lost, expected_buy_token_lost)
        }
        TradeKind::Legacy(_) => (BigRational::zero(), BigRational::zero()),
    };

    let sell_token_lost = &summary.sell_tokens_lost - expected_sell_token_lost;
    let sell_token_lost_limit = inaccuracy_limit * &sell_amount;
    let buy_token_lost = &summary.buy_tokens_lost - expected_buy_token_lost;
    let buy_token_lost_limit = inaccuracy_limit * &buy_amount;

    if sell_token_lost >= sell_token_lost_limit || buy_token_lost >= buy_token_lost_limit {
        return Err(Error::TooInaccurate);
    }

    Ok(Estimate {
        out_amount: summary.out_amount,
        gas: summary.gas_used.as_u64(),
        solver: trade.solver(),
        verified: true,
    })
}

#[derive(Debug)]
pub struct PriceQuery {
    pub sell_token: H160,
    // This should be `BUY_ETH_ADDRESS` if you actually want to trade `ETH`
    pub buy_token: H160,
    pub kind: OrderKind,
    pub in_amount: NonZeroU256,
}

#[derive(thiserror::Error, Debug)]
enum Error {
    /// Verification logic ran successfully but the quote was deemed too
    /// inaccurate to be usable.
    #[error("too inaccurate")]
    TooInaccurate,
    /// Some error caused the simulation to not finish successfully.
    #[error("quote could not be simulated")]
    SimulationFailed(#[from] anyhow::Error),
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        crate::trade_finding::Trade,
        app_data::AppDataHash,
        bigdecimal::FromPrimitive,
        model::order::{BuyTokenDestination, SellTokenSource},
        number::conversions::big_rational_from_decimal_str,
    };

    #[test]
    fn discards_inaccurate_quotes() {
        // let's use 0.5 as the base case to avoid rounding issues introduced by float
        // conversion
        let low_threshold = big_rational_from_decimal_str("0.5").unwrap();
        let high_threshold = big_rational_from_decimal_str("0.51").unwrap();

        let query = PriceQuery {
            in_amount: 1_000.try_into().unwrap(),
            kind: OrderKind::Sell,
            sell_token: H160::zero(),
            buy_token: H160::zero(),
        };

        let sell_more = SettleOutput {
            gas_used: 0.into(),
            out_amount: 2_000.into(),
            buy_tokens_lost: BigRational::from_integer(0.into()),
            sell_tokens_lost: BigRational::from_integer(500.into()),
        };

        let trade = TradeKind::Legacy(Default::default());
        let estimate = ensure_quote_accuracy(&low_threshold, &query, &trade, &sell_more);
        assert!(matches!(estimate, Err(Error::TooInaccurate)));

        // passes with slightly higher tolerance
        let estimate = ensure_quote_accuracy(&high_threshold, &query, &trade, &sell_more);
        assert!(estimate.is_ok());

        let pay_out_more = SettleOutput {
            gas_used: 0.into(),
            out_amount: 2_000.into(),
            buy_tokens_lost: BigRational::from_integer(1_000.into()),
            sell_tokens_lost: BigRational::from_integer(0.into()),
        };

        let estimate = ensure_quote_accuracy(&low_threshold, &query, &trade, &pay_out_more);
        assert!(matches!(estimate, Err(Error::TooInaccurate)));

        // passes with slightly higher tolerance
        let estimate = ensure_quote_accuracy(&high_threshold, &query, &trade, &pay_out_more);
        assert!(estimate.is_ok());

        let sell_less = SettleOutput {
            gas_used: 0.into(),
            out_amount: 2_000.into(),
            buy_tokens_lost: BigRational::from_integer(0.into()),
            sell_tokens_lost: BigRational::from_integer((-500).into()),
        };
        // Ending up with surplus in the buffers is always fine
        let estimate = ensure_quote_accuracy(&low_threshold, &query, &trade, &sell_less);
        assert!(estimate.is_ok());

        let pay_out_less = SettleOutput {
            gas_used: 0.into(),
            out_amount: 2_000.into(),
            buy_tokens_lost: BigRational::from_integer((-1_000).into()),
            sell_tokens_lost: BigRational::from_integer(0.into()),
        };
        // Ending up with surplus in the buffers is always fine
        let estimate = ensure_quote_accuracy(&low_threshold, &query, &trade, &pay_out_less);
        assert!(estimate.is_ok());
    }

    #[test]
    fn ensure_quote_accuracy_with_jit_orders_partial_fills() {
        // Inaccuracy limit of 10%
        let low_threshold = big_rational_from_decimal_str("0.1").unwrap();
        let high_threshold = big_rational_from_decimal_str("0.11").unwrap();

        let sell_token: H160 = H160::from_low_u64_be(1);
        let buy_token: H160 = H160::from_low_u64_be(2);

        // Clearing prices: Token A = 1, Token B = 2
        let mut clearing_prices = HashMap::new();
        clearing_prices.insert(sell_token, U256::from(1u64));
        clearing_prices.insert(buy_token, U256::from(2u64));

        let queries = [
            PriceQuery {
                in_amount: NonZeroU256::new(500u64.into()).unwrap(),
                kind: OrderKind::Sell,
                sell_token,
                buy_token,
            },
            PriceQuery {
                in_amount: NonZeroU256::new(250u64.into()).unwrap(),
                kind: OrderKind::Buy,
                sell_token,
                buy_token,
            },
        ];

        let jit_orders = [
            dto::JitOrder {
                sell_token: buy_token,               // Solver sells Token B
                buy_token: sell_token,               // Solver buys Token A
                executed_amount: U256::from(200u64), // Fills the query partially
                side: dto::Side::Sell,
                sell_amount: U256::from(200u64),
                buy_amount: U256::from(400u64),
                receiver: H160::zero(),
                valid_to: 0,
                app_data: AppDataHash::default(),
                partially_fillable: false,
                sell_token_source: SellTokenSource::Erc20,
                buy_token_destination: BuyTokenDestination::Erc20,
                signature: vec![],
                signing_scheme: SigningScheme::Eip1271,
            },
            dto::JitOrder {
                sell_token: buy_token,               // Solver sell Token B
                buy_token: sell_token,               // Solver buys Token A
                executed_amount: U256::from(400u64), // Fills the query partially
                side: dto::Side::Buy,
                sell_amount: U256::from(200u64),
                buy_amount: U256::from(400u64),
                receiver: H160::zero(),
                valid_to: 0,
                app_data: AppDataHash::default(),
                partially_fillable: false,
                sell_token_source: SellTokenSource::Erc20,
                buy_token_destination: BuyTokenDestination::Erc20,
                signature: vec![],
                signing_scheme: SigningScheme::Eip1271,
            },
        ];

        for (query, jit_order) in queries
            .iter()
            .flat_map(|query| jit_orders.iter().map(move |jit_order| (query, jit_order)))
        {
            let trade_kind = TradeKind::Regular(Trade {
                clearing_prices: clearing_prices.clone(),
                gas_estimate: Some(50_000),
                pre_interactions: vec![],
                interactions: vec![],
                solver: H160::from_low_u64_be(0x1234),
                tx_origin: None,
                jit_orders: vec![jit_order.clone()],
            });

            // The out amount is the total amount of buy token the user gets
            let out_amount = match (&query.kind, &jit_order.side) {
                (OrderKind::Sell, dto::Side::Sell) => U256::from(250u64),
                (OrderKind::Buy, dto::Side::Buy) => U256::from(500u64),
                (OrderKind::Sell, dto::Side::Buy) => U256::from(250u64),
                (OrderKind::Buy, dto::Side::Sell) => U256::from(500u64),
            };

            let summary = SettleOutput {
                gas_used: U256::from(50_000u64),
                out_amount,
                // Settlement contract covers the remaining 50 units of Token B, but to test the 11%
                // limit we lose a bit more.
                buy_tokens_lost: BigRational::from_u64(75).unwrap(),
                // Settlement contract gains 100 units of Token A. To test the 11% limit we gain a
                // bit more.
                sell_tokens_lost: BigRational::from_i64(-150).unwrap(),
            };

            // The summary has 10% inaccuracy
            let estimate = ensure_quote_accuracy(&low_threshold, query, &trade_kind, &summary);
            assert!(matches!(estimate, Err(Error::TooInaccurate)));

            // The summary has less than 11% inaccuracy
            let estimate =
                ensure_quote_accuracy(&high_threshold, query, &trade_kind, &summary).unwrap();
            assert!(estimate.verified);
        }
    }

    #[test]
    fn ensure_quote_accuracy_with_jit_orders_fully_fills() {
        // Inaccuracy limit of 10%
        let low_threshold = big_rational_from_decimal_str("0.1").unwrap();
        let high_threshold = big_rational_from_decimal_str("0.11").unwrap();

        let sell_token: H160 = H160::from_low_u64_be(1);
        let buy_token: H160 = H160::from_low_u64_be(2);

        // Clearing prices: Token A = 1, Token B = 2
        let mut clearing_prices = HashMap::new();
        clearing_prices.insert(sell_token, U256::from(1u64));
        clearing_prices.insert(buy_token, U256::from(2u64));

        let queries = [
            PriceQuery {
                in_amount: NonZeroU256::new(500u64.into()).unwrap(),
                kind: OrderKind::Sell,
                sell_token,
                buy_token,
            },
            PriceQuery {
                in_amount: NonZeroU256::new(250u64.into()).unwrap(),
                kind: OrderKind::Buy,
                sell_token,
                buy_token,
            },
        ];

        let jit_orders = [
            dto::JitOrder {
                sell_token: buy_token,               // Solver sells Token B
                buy_token: sell_token,               // Solver buys Token A
                executed_amount: U256::from(250u64), // Fully fills the query
                side: dto::Side::Sell,
                sell_amount: U256::from(250u64),
                buy_amount: U256::from(500u64),
                receiver: H160::zero(),
                valid_to: 0,
                app_data: AppDataHash::default(),
                partially_fillable: false,
                sell_token_source: SellTokenSource::Erc20,
                buy_token_destination: BuyTokenDestination::Erc20,
                signature: vec![],
                signing_scheme: SigningScheme::Eip1271,
            },
            dto::JitOrder {
                sell_token: buy_token,               // Solver sell Token B
                buy_token: sell_token,               // Solver buys Token A
                executed_amount: U256::from(500u64), // Fully fills the query
                side: dto::Side::Buy,
                sell_amount: U256::from(250u64),
                buy_amount: U256::from(500u64),
                receiver: H160::zero(),
                valid_to: 0,
                app_data: AppDataHash::default(),
                partially_fillable: false,
                sell_token_source: SellTokenSource::Erc20,
                buy_token_destination: BuyTokenDestination::Erc20,
                signature: vec![],
                signing_scheme: SigningScheme::Eip1271,
            },
        ];

        for (query, jit_order) in queries
            .iter()
            .flat_map(|query| jit_orders.iter().map(move |jit_order| (query, jit_order)))
        {
            let trade_kind = TradeKind::Regular(Trade {
                clearing_prices: clearing_prices.clone(),
                gas_estimate: Some(50_000),
                pre_interactions: vec![],
                interactions: vec![],
                solver: H160::from_low_u64_be(0x1234),
                tx_origin: None,
                jit_orders: vec![jit_order.clone()],
            });

            // The out amount is the total amount of buy token the user gets
            let out_amount = match (&query.kind, &jit_order.side) {
                (OrderKind::Sell, dto::Side::Sell) => U256::from(250u64),
                (OrderKind::Buy, dto::Side::Buy) => U256::from(500u64),
                (OrderKind::Sell, dto::Side::Buy) => U256::from(250u64),
                (OrderKind::Buy, dto::Side::Sell) => U256::from(500u64),
            };

            let summary = SettleOutput {
                gas_used: U256::from(50_000u64),
                out_amount,
                // Settlement is not expected to lose any buy tokens, but we lose a bit more to test
                // the inaccuracy limit.
                buy_tokens_lost: BigRational::from_u64(25).unwrap(),
                // Settlement is not expected to gain any sell tokens, but we gain a bit more to
                // test the inaccuracy limit.
                sell_tokens_lost: BigRational::from_i64(-50).unwrap(),
            };

            // The summary has 10% inaccuracy
            let estimate = ensure_quote_accuracy(&low_threshold, query, &trade_kind, &summary);
            assert!(matches!(estimate, Err(Error::TooInaccurate)));

            // The summary has less than 11% inaccuracy
            let estimate =
                ensure_quote_accuracy(&high_threshold, query, &trade_kind, &summary).unwrap();
            assert!(estimate.verified);
        }
    }
}
