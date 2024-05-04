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
        support::{AnyoneAuthenticator, Solver, Trader},
        GPv2Settlement,
        IZeroEx,
        WETH9,
    },
    ethcontract::{errors::DeployError, tokens::Tokenize, BlockNumber, Bytes, H160, H256, U256},
    ethrpc::{current_block::CurrentBlockStream, extensions::StateOverride, Web3},
    maplit::hashmap,
    model::{
        order::{OrderData, OrderKind, BUY_ETH_ADDRESS},
        signature::{Signature, SigningScheme},
    },
    num::BigRational,
    number::{conversions::u256_to_big_rational, nonzero::U256 as NonZeroU256},
    std::sync::Arc,
    web3::{ethabi::Token, types::CallRequest},
};

#[async_trait::async_trait]
pub trait TradeVerifying: Send + Sync + 'static {
    /// Verifies if the proposed [`Trade`] actually fulfills the [`PriceQuery`].
    async fn verify(
        &self,
        query: &PriceQuery,
        verification: &Verification,
        trade: Trade,
    ) -> Result<Estimate>;
}

/// Component that verifies a trade is actually executable by simulating it
/// and determines a price estimate based off of that simulation.
#[derive(Clone)]
pub struct TradeVerifier {
    web3: Web3,
    simulator: Arc<dyn CodeSimulating>,
    code_fetcher: Arc<dyn CodeFetching>,
    block_stream: CurrentBlockStream,
    settlement: GPv2Settlement,
    native_token: H160,
    quote_inaccuracy_limit: BigRational,
    zeroex: Option<IZeroEx>,
}

impl TradeVerifier {
    const DEFAULT_GAS: u64 = 8_000_000;
    const TRADER_IMPL: H160 = addr!("0000000000000000000000000000000000010000");

    pub async fn new(
        web3: Web3,
        simulator: Arc<dyn CodeSimulating>,
        code_fetcher: Arc<dyn CodeFetching>,
        block_stream: CurrentBlockStream,
        settlement: H160,
        native_token: H160,
        quote_inaccuracy_limit: f64,
    ) -> Self {
        let zeroex = match IZeroEx::deployed(&web3).await {
            Ok(instance) => Some(instance),
            Err(DeployError::NotFound(_)) => None,
            Err(err) => panic!("can't find deployed zeroex contract: {err:?}"),
        };

        Self {
            simulator,
            code_fetcher,
            block_stream,
            settlement: GPv2Settlement::at(&web3, settlement),
            native_token,
            quote_inaccuracy_limit: BigRational::from_float(quote_inaccuracy_limit)
                .expect("can represent all finite values"),
            zeroex,
            web3,
        }
    }

    async fn verify_inner(
        &self,
        query: &PriceQuery,
        verification: &Verification,
        trade: &Trade,
    ) -> Result<Estimate, Error> {
        if verification.from.is_zero() {
            // Don't waste time on common simulations which will always fail.
            return Err(anyhow::anyhow!("trader is zero address").into());
        }

        let start = std::time::Instant::now();

        // Use regular solver address or special address required by RFQ interaction of
        // the trade.
        let origin_required_by_trade = self
            .zeroex
            .as_ref()
            .and_then(|zeroex| origin_required_by_rfq_interaction(trade, zeroex));
        let solver = dummy_contract!(Solver, origin_required_by_trade.unwrap_or(trade.solver));

        let settlement = encode_settlement(query, verification, trade, self.native_token);
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
            OrderKind::Buy => trade.out_amount,
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

        let block = self.block_stream.borrow().number;
        if origin_required_by_trade.is_some_and(|origin| origin != trade.solver) {
            // If the trade requires a special tx.origin we also need to fake the
            // authenticator and tx origin balance.
            let (authenticator, balance) = futures::join!(
                self.settlement.authenticator().call(),
                self.web3
                    .eth()
                    .balance(trade.solver, Some(BlockNumber::Number(block.into())))
            );
            overrides.insert(
                authenticator.unwrap(),
                StateOverride {
                    code: Some(deployed_bytecode!(AnyoneAuthenticator)),
                    balance: Some(balance.unwrap()),
                    ..Default::default()
                },
            );
        }

        let trader_impl = self
            .code_fetcher
            .code(verification.from)
            .await
            .context("failed to fetch trader code")
            .map_err(Error::SimulationFailed)?;
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

        let output = self
            .simulator
            .simulate(call, overrides, Some(block))
            .await
            .context("failed to simulate quote")
            .map_err(Error::SimulationFailed);

        // TODO remove as soon as quoters stop signign RFQ orders for `tx.origin:
        // 0x0000`
        if let Err(err) = &output {
            // Check if simulation failed only because the trade used a zeroex RFQ order
            // that expects the tx origin to be the zero address.
            // Those simulations will always fail but if you'd want to game the system it
            // would be pretty simple by signing an RFQ order you later wouldn't execute
            // so we just pretend the simulation was successful.
            // Simulation failures for any other tx origin should actually lead to an
            // error since we can mock the expected conditions reasonably well so a
            // simulation failure definitely indicates a broken quote for those.
            if origin_required_by_trade == Some(H160::zero()) {
                let estimate = Estimate {
                    out_amount: trade.out_amount,
                    gas: trade.gas_estimate.context("no gas estimate")?,
                    solver: trade.solver,
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

        let summary = SettleOutput::decode(&output?, query.kind)
            .context("could not decode simulation output")
            .map_err(Error::SimulationFailed)?;
        tracing::debug!(
            lost_buy_amount = %summary.buy_tokens_diff,
            lost_sell_amount = %summary.sell_tokens_diff,
            gas_diff = ?trade.gas_estimate.unwrap_or_default().abs_diff(summary.gas_used.as_u64()),
            time = ?start.elapsed(),
            promised_out_amount = ?trade.out_amount,
            verified_out_amount = ?summary.out_amount,
            promised_gas = trade.gas_estimate,
            verified_gas = ?summary.gas_used,
            out_diff = ?trade.out_amount.abs_diff(summary.out_amount),
            ?query,
            ?verification,
            "verified quote",
        );

        ensure_quote_accuracy(&self.quote_inaccuracy_limit, query, trade.solver, &summary)
    }
}

// TODO delete as soon as we can convince solvers to simply make their solver
// address the required `tx.origin` because we can get rid of a lot of trickery
// then.
//
/// If the trade uses some zeroex RFQ interaction the function returns the
/// address the orders expects the `tx.origin` to be.
/// If multiple interactions require different `tx.origin` the first one will be
/// returned. In that case the trade simulation will still fail because all
/// interactions will get executed in the same tx which can only have a single
/// origin so at least one interaction will revert.
/// Returns `None` on any error which is okay since we'll detect the same error
/// during the simulation as well.
fn origin_required_by_rfq_interaction(trade: &Trade, zeroex: &IZeroEx) -> Option<H160> {
    /// Public functions on [`IZeroEx`] which are used to settle RFQ orders.
    const RFQ_FUNCTIONS: &[&str] = &["fillRfqOrder", "fillOrKillRfqOrder"];
    let abi = &IZeroEx::raw_contract().interface.abi;

    let calls_function = |i: &Interaction, fun| {
        let signature = &abi.function(fun).unwrap().short_signature();
        i.data.starts_with(signature.as_slice())
    };

    trade.interactions.iter().find_map(|i| {
        let is_rfq_interaction =
            i.target == zeroex.address() && RFQ_FUNCTIONS.iter().any(|fun| calls_function(i, fun));
        if !is_rfq_interaction {
            return None;
        }
        // strip function signature bytes and only keep function arguments
        let arguments = &i.data[4..];

        // `fillRfqOrder()` and `fillOrKillRfqOrder()` have the same signature so we can
        // decode their calldata the same way.
        let tokenized = abi
            .function("fillRfqOrder")
            .ok()?
            .decode_input(arguments)
            .ok()?;
        let (order, _, _) =
            <(RfqOrder, ZeroExSignature, u128)>::from_token(Token::Tuple(tokenized)).ok()?;

        // tx_origin
        Some(order.6)
    })
}

type RfqOrder = (
    // maker_token
    H160,
    // taker_token
    H160,
    // maker_amount
    u128,
    // taker_amount
    u128,
    // maker
    H160,
    // taker
    H160,
    // txOrigin
    H160,
    // pool
    H256,
    // expiry
    u64,
    // salt
    U256,
);

type ZeroExSignature = (
    // signature_type
    u8,
    // v
    u8,
    // r
    H256,
    // s
    H256,
);

#[async_trait::async_trait]
impl TradeVerifying for TradeVerifier {
    async fn verify(
        &self,
        query: &PriceQuery,
        verification: &Verification,
        trade: Trade,
    ) -> Result<Estimate> {
        match self.verify_inner(query, verification, &trade).await {
            Ok(verified) => Ok(verified),
            Err(Error::SimulationFailed(err)) => match trade.gas_estimate {
                Some(gas) => {
                    let estimate = Estimate {
                        out_amount: trade.out_amount,
                        gas,
                        solver: trade.solver,
                        verified: false,
                    };
                    tracing::warn!(
                        ?err,
                        estimate = ?trade,
                        "failed verification; returning unferified estimate"
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
    buy_tokens_diff: BigRational,
    /// Difference in sell tokens of the settlement contract before and after
    /// the trade.
    sell_tokens_diff: BigRational,
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
            buy_tokens_diff: settlement_buy_balance_before - settlement_buy_balance_after,
            sell_tokens_diff: settlement_sell_balance_before - settlement_sell_balance_after,
        })
    }
}

/// Returns an error if settling the quote would require using too much of the
/// settlement contract buffers.
fn ensure_quote_accuracy(
    inaccuracy_limit: &BigRational,
    query: &PriceQuery,
    solver: H160,
    summary: &SettleOutput,
) -> Result<Estimate, Error> {
    // amounts verified by the simulation
    let (sell_amount, buy_amount) = match query.kind {
        OrderKind::Buy => (summary.out_amount, query.in_amount.get()),
        OrderKind::Sell => (query.in_amount.get(), summary.out_amount),
    };

    if summary.sell_tokens_diff >= inaccuracy_limit * u256_to_big_rational(&sell_amount)
        || summary.buy_tokens_diff >= inaccuracy_limit * u256_to_big_rational(&buy_amount)
    {
        return Err(Error::TooInaccurate);
    }

    Ok(Estimate {
        out_amount: summary.out_amount,
        gas: summary.gas_used.as_u64(),
        solver,
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
    use super::*;

    #[test]
    fn discards_inaccurate_quotes() {
        // let's use 0.5 as the base case to avoid rounding issues introduced by float
        // conversion
        let low_threshold = BigRational::from_float(0.5).unwrap();
        let high_threshold = BigRational::from_float(0.51).unwrap();

        let query = PriceQuery {
            in_amount: 1_000.try_into().unwrap(),
            kind: OrderKind::Sell,
            sell_token: H160::zero(),
            buy_token: H160::zero(),
        };

        let sell_more = SettleOutput {
            gas_used: 0.into(),
            out_amount: 2_000.into(),
            buy_tokens_diff: BigRational::from_integer(0.into()),
            sell_tokens_diff: BigRational::from_integer(500.into()),
        };

        let estimate = ensure_quote_accuracy(&low_threshold, &query, H160::zero(), &sell_more);
        assert!(matches!(estimate, Err(Error::TooInaccurate)));

        // passes with slightly higher tolerance
        let estimate = ensure_quote_accuracy(&high_threshold, &query, H160::zero(), &sell_more);
        assert!(estimate.is_ok());

        let pay_out_more = SettleOutput {
            gas_used: 0.into(),
            out_amount: 2_000.into(),
            buy_tokens_diff: BigRational::from_integer(1_000.into()),
            sell_tokens_diff: BigRational::from_integer(0.into()),
        };

        let estimate = ensure_quote_accuracy(&low_threshold, &query, H160::zero(), &pay_out_more);
        assert!(matches!(estimate, Err(Error::TooInaccurate)));

        // passes with slightly higher tolerance
        let estimate = ensure_quote_accuracy(&high_threshold, &query, H160::zero(), &pay_out_more);
        assert!(estimate.is_ok());

        let sell_less = SettleOutput {
            gas_used: 0.into(),
            out_amount: 2_000.into(),
            buy_tokens_diff: BigRational::from_integer(0.into()),
            sell_tokens_diff: BigRational::from_integer((-500).into()),
        };
        // Ending up with surplus in the buffers is always fine
        let estimate = ensure_quote_accuracy(&low_threshold, &query, H160::zero(), &sell_less);
        assert!(estimate.is_ok());

        let pay_out_less = SettleOutput {
            gas_used: 0.into(),
            out_amount: 2_000.into(),
            buy_tokens_diff: BigRational::from_integer((-1_000).into()),
            sell_tokens_diff: BigRational::from_integer(0.into()),
        };
        // Ending up with surplus in the buffers is always fine
        let estimate = ensure_quote_accuracy(&low_threshold, &query, H160::zero(), &pay_out_less);
        assert!(estimate.is_ok());
    }
}
