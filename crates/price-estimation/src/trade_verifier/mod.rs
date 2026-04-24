use {
    super::{Estimate, Verification},
    crate::{
        trade_finding::{
            Interaction,
            QuoteExecution,
            TradeKind,
            external::dto::{self, Side},
            map_interactions_data,
        },
        trade_verifier::code_fetching::CodeFetching,
    },
    ::alloy::sol_types::SolCall,
    alloy::{
        primitives::{Address, U256, address, aliases::I512, map::AddressMap},
        rpc::types::{eth::state::StateOverride, state::AccountOverride},
    },
    anyhow::{Context, Result},
    balance_overrides::{BalanceOverrideRequest, BalanceOverriding},
    bigdecimal::BigDecimal,
    contracts::{
        GPv2Settlement,
        support::{AnyoneAuthenticator, Solver, Spardose, Trader},
    },
    ethrpc::Web3,
    model::{
        DomainSeparator,
        order::{OrderData, OrderKind},
        signature::{Signature, SigningScheme},
    },
    num::BigRational,
    number::{
        conversions::{
            big_decimal_to_big_rational,
            i512_to_big_rational,
            i512_to_u256,
            u256_to_big_rational,
        },
        nonzero::NonZeroU256,
    },
    simulator::{
        encoding::{EncodedTrade, InteractionEncoding, encode_trade},
        swap_simulator::{EncodedSwap, SwapSimulator, TradeEncoding},
        tenderly::{self},
    },
    std::{
        collections::{HashMap, HashSet},
        sync::Arc,
    },
    tracing::instrument,
};

pub mod code_fetching;

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
    tenderly: Option<Arc<dyn tenderly::Api>>,
    simulator: SwapSimulator,
    code_fetcher: Arc<dyn CodeFetching>,
    balance_overrides: Arc<dyn BalanceOverriding>,
    settlement: GPv2Settlement::Instance,
    quote_inaccuracy_limit: BigRational,
    tokens_without_verification: HashSet<Address>,
    min_gas_amount_for_unverified_quotes: u32,
    max_gas_amount_for_unverified_quotes: u32,
}

impl TradeVerifier {
    const SPARDOSE: Address = address!("0000000000000000000000000000000000020000");
    const TRADER_IMPL: Address = address!("0000000000000000000000000000000000010000");

    #[expect(clippy::too_many_arguments)]
    pub async fn new(
        web3: Web3,
        tenderly: Option<Arc<dyn tenderly::Api>>,
        simulator: SwapSimulator,
        code_fetcher: Arc<dyn CodeFetching>,
        balance_overrides: Arc<dyn BalanceOverriding>,
        settlement: Address,
        quote_inaccuracy_limit: BigDecimal,
        tokens_without_verification: HashSet<Address>,
        min_gas_amount_for_unverified_quotes: u32,
        max_gas_amount_for_unverified_quotes: u32,
    ) -> Result<Self> {
        assert!(
            min_gas_amount_for_unverified_quotes <= max_gas_amount_for_unverified_quotes,
            "gas floor ({min_gas_amount_for_unverified_quotes}) exceeds gas ceiling \
             ({max_gas_amount_for_unverified_quotes}) for unverified quotes"
        );
        let settlement_contract =
            GPv2Settlement::GPv2Settlement::new(settlement, web3.provider.clone());
        Ok(Self {
            tenderly,
            simulator,
            code_fetcher,
            balance_overrides,
            settlement: settlement_contract,
            quote_inaccuracy_limit: big_decimal_to_big_rational(&quote_inaccuracy_limit),
            tokens_without_verification,
            min_gas_amount_for_unverified_quotes,
            max_gas_amount_for_unverified_quotes,
        })
    }

    async fn verify_inner(
        &self,
        query: &PriceQuery,
        mut verification: Verification,
        trade: &TradeKind,
        out_amount: &U256,
    ) -> Result<Estimate, Error> {
        let start = std::time::Instant::now();
        let overrides = self
            .prepare_state_overrides(&mut verification, query, trade)
            .await?;

        // Use `tx_origin` if response indicates that a special address is needed for
        // the simulation to pass. Otherwise just use the solver address.
        let solver_address = trade.tx_origin().unwrap_or(trade.solver());

        let (tokens, clearing_prices) = match trade {
            TradeKind::Legacy(_) => {
                let tokens = vec![query.sell_token, query.buy_token];
                let prices = match query.kind {
                    OrderKind::Sell => {
                        vec![*out_amount, query.in_amount.get()]
                    }
                    OrderKind::Buy => {
                        vec![query.in_amount.get(), *out_amount]
                    }
                };
                (tokens, prices)
            }
            TradeKind::Regular(trade) => trade.clearing_prices.iter().unzip(),
        };

        let (sell_amount, buy_amount) = match query.kind {
            OrderKind::Sell => (query.in_amount, *out_amount),
            OrderKind::Buy => (
                NonZeroU256::try_from(*out_amount).context("computed sell amount is zero")?,
                query.in_amount.get(),
            ),
        };
        let simulator_query = simulator::swap_simulator::Query {
            sell_token: query.sell_token,
            buy_token: query.buy_token,
            kind: query.kind,
            sell_amount,
            buy_amount,
            receiver: verification.receiver,
            sell_token_source: verification.sell_token_source,
            buy_token_destination: verification.buy_token_destination,
            from: verification.from,
            tx_origin: trade.tx_origin(),
            solver: solver_address,
            tokens: tokens.clone(),
            clearing_prices,
            wrappers: Default::default(),
        };

        let mut swap = self
            .simulator
            .fake_swap(&simulator_query, TradeEncoding::Disadvantageous)
            .await
            .map_err(Error::SimulationFailed)?;

        swap.overrides.extend(overrides);

        let mut pre_interactions = verification
                .pre_interactions
                .iter()
                // pre_interactions introduced by the solver
                .chain(trade.pre_interactions())
                .map(InteractionEncoding::encode)
                .collect::<Vec<_>>();

        // Join custom pre_interactions in the following order:
        // pre_interactions, trade setup interaction, encoded swap pre interactions
        pre_interactions.extend([self
            .trade_setup_interaction(out_amount, &verification, query, trade)
            .encode()]);
        pre_interactions.extend(swap.settlement.interactions.pre);
        swap.settlement.interactions.pre = pre_interactions;

        // Join interactions introduced by the solver, set up in the following order:
        // trade interactions, encoded swap interactions
        let interactions = trade.interactions().map(InteractionEncoding::encode);
        swap.settlement.interactions.main = interactions
            .into_iter()
            .chain(swap.settlement.interactions.main)
            .collect();

        // Join post interactions in the following order:
        // encoded swap post interactions, verification post interactions,
        let post_interactions = verification
            .post_interactions
            .iter()
            .map(InteractionEncoding::encode);
        swap.settlement.interactions.post = swap
            .settlement
            .interactions
            .post
            .into_iter()
            .chain(post_interactions)
            .collect();

        add_balance_queries(&mut swap, query, &verification);

        if let TradeKind::Regular(trade) = trade {
            swap.settlement.trades.extend(encode_jit_orders(
                &trade.jit_orders,
                &swap.settlement.tokens,
                &self.simulator.domain_separator,
            )?);
        }
        let block = *self.simulator.current_block.borrow();
        let output = self
            .simulator
            .simulate_swap_with_solver(swap, block)
            .await?;

        if let Some(tenderly) = &self.tenderly
            && let Err(err) =
                tenderly.log_simulation_command(output.tx, output.overrides, block.number.into())
        {
            tracing::debug!(?err, "could not log tenderly simulation command");
        }
        let output = output
            .result
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
            if trade.tx_origin() == Some(Address::ZERO) {
                let estimate = Estimate {
                    out_amount: *out_amount,
                    gas: trade.gas_estimate().context("no gas estimate")?,
                    solver: trade.solver(),
                    verified: true,
                    execution: QuoteExecution {
                        interactions: map_interactions_data(trade.interactions()),
                        pre_interactions: map_interactions_data(trade.pre_interactions()),
                        jit_orders: trade.jit_orders().cloned().collect(),
                    },
                };
                tracing::warn!(
                    ?estimate,
                    ?err,
                    "quote used invalid zeroex RFQ order; pass verification anyway"
                );
                return Ok(estimate);
            }
        };

        let mut summary = SettleOutput::from_swap(output?, query.kind, &tokens)?;

        {
            // Quote accuracy gets determined by how many tokens had to be paid out of the
            // settlement buffers to make the quote happen. When the settlement contract
            // itself is the trader or receiver these values need to be adjusted slightly.
            let (sell_amount, buy_amount) = match query.kind {
                OrderKind::Sell => (I512::from(query.in_amount.get()), summary.out_amount),
                OrderKind::Buy => (summary.out_amount, I512::from(query.in_amount.get())),
            };

            // It looks like the contract lost a lot of sell tokens but only because it was
            // the trader and had to pay for the trade. Adjust tokens lost downward.
            if verification.from == *self.settlement.address() {
                summary
                    .tokens_lost
                    .entry(query.sell_token)
                    .and_modify(|balance| *balance -= i512_to_big_rational(&sell_amount));
            }
            // It looks like the contract gained a lot of buy tokens (negative loss) but
            // only because it was the receiver and got the payout. Adjust the tokens lost
            // upward.
            if verification.receiver == *self.settlement.address() {
                summary
                    .tokens_lost
                    .entry(query.buy_token)
                    .and_modify(|balance| *balance += i512_to_big_rational(&buy_amount));
            }

            // The swap simulation computes the out_amount like this:
            // sell order => receiver_buy_balance_before - receiver_buy_balance_after
            // buy_order => trader_sell_balance_after - trader_sell_balance_before
            //
            // The trade verification assumes that the sell tokens don't flow back into
            // the same account.
            // However, in case of sell=buy where the receiver is also the owner, this
            // assumption is broken. The balance is only ever getting smaller, as the
            // trader will always get less tokens out, which causes the above calculations
            // to result in 0 or (more likely) negative values.
            //
            // Example sell order:
            // Trader having 1 ETH in their account, selling 0.3 ETH, with tx hooks cost of
            // 0.1 ETH: in_amount = 0.3 ETH
            // trader_balance_before = 1 ETH
            // trader_balance_after = 0.9 ETH
            // out_amount = 0.9 ETH - 1 ETH = -0.1 ETH
            // The correct out_amount = 0.3 ETH (input) + (-0.1ETH) (out_amount) = 0.2 ETH
            //
            // Meaning they can sell 0.3 ETH for 0.2 ETH, considering the costs
            //
            // Example buy order:
            // Trader having 1 ETH in their account, buying 1 wei, with tx hooks cost of 0.1
            // ETH in_amount = 1 wei
            // trader_balance_before = 1 ETH
            // trader_balance_after = 0.9 ETH
            // out_amount = 1 ETH - 0.9 ETH = 0.1 ETH
            // The correct out_amount = 1 wei (input) + 0.1 ETH (out_amount) = 0.1000...1
            // ETH
            //
            // Meaning they can buy 1 wei for 0.1ETH + 1 wei, considering the costs
            //
            // The general formula being: correct_out_amount = query.input + out_amount
            let owner_is_receiver =
                verification.receiver.is_zero() || verification.receiver == verification.from;
            if query.sell_token == query.buy_token && owner_is_receiver {
                summary.out_amount = I512::from(query.in_amount.get()) + summary.out_amount;
            } else if summary.out_amount < I512::ZERO {
                tracing::debug!("Trade out amount is negative");
                return Err(Error::BuffersPayForOrder);
            }
        }

        tracing::debug!(
            tokens_lost = ?summary.tokens_lost,
            gas_diff = ?trade.gas_estimate().unwrap_or_default().abs_diff(summary.gas_used.saturating_to()),
            time = ?start.elapsed(),
            promised_out_amount = ?out_amount,
            verified_out_amount = ?summary.out_amount,
            promised_gas = trade.gas_estimate(),
            verified_gas = ?summary.gas_used,
            out_diff = ?(I512::from(*out_amount) - summary.out_amount).abs(),
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
        verification: &mut Verification,
        query: &PriceQuery,
        trade: &TradeKind,
    ) -> Result<StateOverride> {
        let mut overrides = AddressMap::default();

        // Provide mocked balances if possible to the spardose to allow it to
        // give some balances to the trader in order to verify trades even for
        // owners without balances. Note that we use a separate account for
        // funding to not interfere with the settlement process. This allows the
        // simulation to conditionally transfer the balance only when it is
        // safe to mock the trade pre-conditions on behalf of the user and to
        // not alter solver balances which may be used during settlement. We use
        // a similar strategy for determining whether or not to set approvals on
        // behalf of the trader.
        let needed = match query.kind {
            OrderKind::Sell => query.in_amount.get(),
            OrderKind::Buy => trade.out_amount(
                &query.buy_token,
                &query.sell_token,
                &query.in_amount.get(),
                &query.kind,
            )?,
        };
        match self
            .balance_overrides
            .state_override(BalanceOverrideRequest {
                token: query.sell_token,
                holder: Self::SPARDOSE,
                amount: spardose_amount_with_buffer(needed),
            })
            .await
        {
            Some((token, solver_balance_override)) => {
                tracing::trace!(?solver_balance_override, "solver balance override enabled");
                overrides.insert(token, solver_balance_override);

                if verification.from.is_zero() {
                    verification.from = Address::random();
                    tracing::debug!(
                        trader = ?verification.from,
                        "use random trader address with fake balances"
                    );
                }
            }
            _ => {
                tracing::warn!(
                    sell_token = ?query.sell_token,
                    "could not set spardose balance override for sell token; trade \
                     verification will rely on the trader's real balance"
                );
                if verification.from.is_zero() {
                    anyhow::bail!("trader is zero address and balances can not be faked");
                }
            }
        }

        // Set up mocked trader.
        overrides.insert(
            verification.from,
            AccountOverride {
                code: Some(Trader::Trader::DEPLOYED_BYTECODE.clone()),
                ..Default::default()
            },
        );

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
                AccountOverride {
                    code: Some(trader_impl),
                    ..Default::default()
                },
            );
        }

        // Setup the funding contract override. Regardless of whether or not the
        // contract has funds, it needs to exist in order to not revert
        // simulations (Solidity reverts on attempts to call addresses without
        // any code).
        overrides.insert(
            Self::SPARDOSE,
            AccountOverride {
                code: Some(Spardose::Spardose::DEPLOYED_BYTECODE.clone()),
                ..Default::default()
            },
        );

        // Set up mocked solver.
        let solver_override = AccountOverride {
            code: Some(Solver::Solver::DEPLOYED_BYTECODE.clone()),
            // Allow solver simulations to proceed even if the real account holds no ETH.
            // The number is obscenely large, but not max to avoid potential overflows.
            // We had this set to eth(1), but some simulations require more than that on non-ETH
            // netowrks e.g. polygon so it led to reverts.
            balance: Some(U256::MAX / U256::from(2)),
            ..Default::default()
        };

        // If the trade requires a special tx.origin we also need to fake the
        // authenticator.
        if trade
            .tx_origin()
            .is_some_and(|origin| origin != trade.solver())
        {
            let authenticator = self
                .settlement
                .authenticator()
                .call()
                .await
                .context("could not fetch authenticator")?;
            overrides.insert(
                authenticator,
                AccountOverride {
                    code: Some(AnyoneAuthenticator::AnyoneAuthenticator::DEPLOYED_BYTECODE.clone()),
                    ..Default::default()
                },
            );
        }

        overrides.insert(trade.tx_origin().unwrap_or(trade.solver()), solver_override);

        Ok(overrides)
    }

    /// Create interaction that sets up the trade right before transfering
    /// funds. This interaction does nothing if the user-provided
    /// pre-interactions already set everything up (e.g. approvals,
    /// balances). That way we can correctly verify quotes with or without
    /// these user pre-interactions with helpful error messages.
    fn trade_setup_interaction(
        &self,
        out_amount: &U256,
        verification: &Verification,
        query: &PriceQuery,
        trade: &TradeKind,
    ) -> Interaction {
        let sell_amount = match query.kind {
            OrderKind::Sell => query.in_amount.get(),
            OrderKind::Buy => *out_amount,
        };
        let setup_call = Solver::Solver::ensureTradePreconditionsCall {
            trader: verification.from,
            settlementContract: *self.settlement.address(),
            sellToken: query.sell_token,
            sellAmount: sell_amount,
            nativeToken: self.simulator.native_token,
            spardose: Self::SPARDOSE,
        }
        .abi_encode();
        Interaction {
            target: trade.solver(),
            value: U256::ZERO,
            data: setup_call,
        }
    }
}

#[async_trait::async_trait]
impl TradeVerifying for TradeVerifier {
    #[instrument(skip_all)]
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

        let unverified_result = trade
            .gas_estimate()
            .map(|gas| {
                let gas = gas.clamp(
                    self.min_gas_amount_for_unverified_quotes as u64,
                    self.max_gas_amount_for_unverified_quotes as u64,
                );

                Estimate {
                    out_amount,
                    gas,
                    solver: trade.solver(),
                    verified: false,
                    execution: QuoteExecution {
                        interactions: map_interactions_data(trade.interactions()),
                        pre_interactions: map_interactions_data(trade.pre_interactions()),
                        jit_orders: trade.jit_orders().cloned().collect(),
                    },
                }
            })
            .context("solver provided no gas estimate");

        let skip_verification = [query.buy_token, query.sell_token]
            .iter()
            .any(|token| self.tokens_without_verification.contains(token));
        if skip_verification {
            tracing::debug!(estimate = ?unverified_result, "quote verification skipped");
            return unverified_result;
        }

        match self
            .verify_inner(query, verification.clone(), &trade, &out_amount)
            .await
        {
            Ok(verified) => Ok(verified),
            Err(err) => {
                // For some tokens it's not possible to provide verifiable calldata in the
                // quote (e.g. when they require the use of proprietary APIs which don't give
                // out calldata willy nilly).
                //
                // Since you can't magically make up calldata that makes your quote verifiable
                // solvers don't provide any call data in those cases.
                // This has 2 possible outcomes:
                // 1. the settlement contract has enough buy_tokens to pay for the order =>
                //    Error::BuffersPayForOrder
                // 2. not enough buy tokens in buffer => error::SimulationFailure
                //
                // To make handling of these quotes more predictable we'll only discard
                // `Error::BufferPayForOrder` errors if the solver actually tried to provide a
                // an execution plan but it's just not correct. In all other cases we just flag
                // the solution as unverified but let it pass.
                let has_execution_plan = trade.has_execution_plan();
                if has_execution_plan && matches!(err, Error::BuffersPayForOrder) {
                    tracing::debug!(
                        has_execution_plan,
                        "discarding quote because buffers pay for order"
                    );
                    Err(err.into())
                } else {
                    tracing::debug!(estimate = ?unverified_result, ?err, "quote verification failed");
                    unverified_result
                }
            }
        }
    }
}

fn add_balance_queries(swap: &mut EncodedSwap, query: &PriceQuery, verification: &Verification) {
    let (token, owner) = match query.kind {
        // track how much `buy_token` the `receiver` actually got
        OrderKind::Sell => {
            let receiver = match verification.receiver.is_zero() {
                // Settlement contract sends fund to owner if receiver is the 0 address.
                true => verification.from,
                false => verification.receiver,
            };

            (query.buy_token, receiver)
        }
        // track how much `sell_token` the `from` address actually spent
        OrderKind::Buy => (query.sell_token, verification.from),
    };
    let query_balance_call = Solver::Solver::storeBalanceCall {
        token,
        owner,
        countGas: true,
    }
    .abi_encode();

    let interaction = (swap.solver, U256::ZERO, query_balance_call.into());

    // query balance query at the end of pre-interactions
    swap.settlement.interactions.pre.push(interaction.clone());
    // query balance right after we payed out all `buy_token`
    swap.settlement.interactions.post.insert(0, interaction);
}

/// Analyzed output of `Solver::settle` smart contract call.
#[derive(Debug)]
struct SettleOutput {
    /// Gas used for the `settle()` call.
    gas_used: U256,
    /// `out_amount` perceived by the trader (sell token for buy orders or buy
    /// token for sell order)
    out_amount: alloy::primitives::aliases::I512,
    /// Tokens difference of the settlement contract before and after the trade.
    tokens_lost: HashMap<Address, BigRational>,
}

impl SettleOutput {
    fn from_swap(
        Solver::Solver::swapReturn {
            gasUsed,
            queriedBalances,
        }: Solver::Solver::swapReturn,
        kind: OrderKind,
        tokens_vec: &[Address],
    ) -> Result<Self> {
        // The balances are stored in the following order:
        // [...tokens_before, user_balance_before, user_balance_after, ...tokens_after]
        let mut i = 0;
        let mut tokens_lost = HashMap::new();
        // Get settlement contract balances before the trade
        for token in tokens_vec.iter() {
            // TODO: add alloy support to the numeric functions
            let balance_before = u256_to_big_rational(&queriedBalances[i]);
            tokens_lost.insert(*token, balance_before);
            i += 1;
        }

        let trader_balance_before = I512::from(queriedBalances[i]);
        let trader_balance_after = I512::from(queriedBalances[i + 1]);
        i += 2;

        // Get settlement contract balances after the trade
        for token in tokens_vec.iter() {
            let balance_after = u256_to_big_rational(&queriedBalances[i]);
            tokens_lost
                .entry(*token)
                .and_modify(|balance_before| *balance_before -= balance_after);
            i += 1;
        }

        let out_amount = match kind {
            // for sell orders we track the buy_token amount which increases during the settlement
            OrderKind::Sell => trader_balance_after - trader_balance_before,
            // for buy orders we track the sell_token amount which decreases during the settlement
            OrderKind::Buy => trader_balance_before - trader_balance_after,
        };

        Ok(SettleOutput {
            gas_used: gasUsed,
            out_amount,
            tokens_lost,
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
        OrderKind::Buy => (summary.out_amount, I512::from(query.in_amount.get())),
        OrderKind::Sell => (I512::from(query.in_amount.get()), summary.out_amount),
    };
    let (sell_amount, buy_amount) = (
        i512_to_big_rational(&sell_amount),
        i512_to_big_rational(&buy_amount),
    );
    let sell_token_lost_limit = inaccuracy_limit * &sell_amount;
    let buy_token_lost_limit = inaccuracy_limit * &buy_amount;

    let sell_token_lost = summary
        .tokens_lost
        .get(&query.sell_token)
        .context("summary sell token is missing")?;
    let buy_token_lost = summary
        .tokens_lost
        .get(&query.buy_token)
        .context("summary buy token is missing")?;

    if (*sell_token_lost >= sell_token_lost_limit) || (*buy_token_lost >= buy_token_lost_limit) {
        return Err(Error::BuffersPayForOrder);
    }

    Ok(Estimate {
        out_amount: i512_to_u256(&summary.out_amount)?,
        gas: summary.gas_used.saturating_to(),
        solver: trade.solver(),
        verified: true,
        execution: QuoteExecution {
            interactions: map_interactions_data(trade.interactions()),
            pre_interactions: map_interactions_data(trade.pre_interactions()),
            jit_orders: trade.jit_orders().cloned().collect(),
        },
    })
}

#[derive(Debug)]
pub struct PriceQuery {
    pub sell_token: Address,
    // This should be `BUY_ETH_ADDRESS` if you actually want to trade `ETH`
    pub buy_token: Address,
    pub kind: OrderKind,
    pub in_amount: NonZeroU256,
}

pub fn encode_jit_orders(
    jit_orders: &[dto::JitOrder],
    tokens: &[Address],
    domain_separator: &DomainSeparator,
) -> Result<Vec<EncodedTrade>> {
    jit_orders
        .iter()
        .map(|jit_order| {
            let order_data = OrderData {
                sell_token: jit_order.sell_token,
                buy_token: jit_order.buy_token,
                receiver: Some(jit_order.receiver),
                sell_amount: jit_order.sell_amount,
                buy_amount: jit_order.buy_amount,
                valid_to: jit_order.valid_to,
                app_data: jit_order.app_data,
                fee_amount: U256::ZERO,
                kind: match &jit_order.side {
                    Side::Buy => OrderKind::Buy,
                    Side::Sell => OrderKind::Sell,
                },
                partially_fillable: jit_order.partially_fillable,
                sell_token_balance: jit_order.sell_token_source,
                buy_token_balance: jit_order.buy_token_destination,
            };
            let (owner, signature) =
                recover_jit_order_owner(jit_order, &order_data, domain_separator)?;

            Ok(encode_trade(
                &order_data,
                &signature,
                owner,
                // the tokens set length is small so the linear search is acceptable
                tokens
                    .iter()
                    .position(|token| *token == jit_order.sell_token)
                    .context("missing jit order sell token index")?,
                tokens
                    .iter()
                    .position(|token| *token == jit_order.buy_token)
                    .context("missing jit order buy token index")?,
                jit_order.executed_amount,
            ))
        })
        .collect::<Result<Vec<EncodedTrade>>>()
}

/// Recovers the owner and signature from a `JitOrder`.
fn recover_jit_order_owner(
    jit_order: &dto::JitOrder,
    order_data: &OrderData,
    domain_separator: &DomainSeparator,
) -> Result<(Address, Signature)> {
    let (owner, signature) = match jit_order.signing_scheme {
        SigningScheme::Eip1271 => {
            let (owner, signature) = jit_order.signature.split_at(20);
            let owner = Address::from_slice(owner);
            let signature = Signature::from_bytes(jit_order.signing_scheme, signature)?;
            (owner, signature)
        }
        SigningScheme::PreSign => {
            let owner = Address::from_slice(&jit_order.signature);
            let signature = Signature::from_bytes(jit_order.signing_scheme, Vec::new().as_slice())?;
            (owner, signature)
        }
        _ => {
            let signature = Signature::from_bytes(jit_order.signing_scheme, &jit_order.signature)?;
            let owner = signature
                .recover(domain_separator, &order_data.hash_struct())?
                .context("could not recover the owner")?
                .signer;
            (owner, signature)
        }
    };
    Ok((owner, signature))
}

#[derive(thiserror::Error, Debug)]
enum Error {
    /// Verification logic ran successfully but the quote was deemed too
    /// inaccurate because too many buy tokens came from the settlement
    /// contract's buffers.
    #[error("buffers pay for order")]
    BuffersPayForOrder,
    /// Some error caused the simulation to not finish successfully.
    #[error("quote could not be simulated")]
    SimulationFailed(#[from] anyhow::Error),
}

/// Spardose gets `needed` plus a 1% headroom, floored at 1 wei so the
/// 1-wei boundary is still covered for small amounts where `needed / 100`
/// truncates to 0. The buffer absorbs rounding or per-block accrual
/// (aToken, rebasing, tiny fee-on-transfer) between our state_override
/// read and the sim's execution. Spardose is a throwaway donor, so
/// overshoot has no cost.
fn spardose_amount_with_buffer(needed: U256) -> U256 {
    let buffer = std::cmp::max(U256::ONE, needed / U256::from(100u64));
    needed.saturating_add(buffer)
}

#[cfg(test)]
mod tests {
    use {super::*, U256, maplit::hashmap, std::str::FromStr};

    #[test]
    fn spardose_amount_applies_1pct_overshoot() {
        assert_eq!(
            spardose_amount_with_buffer(U256::from(1_000_000_000_000_000_000u128)),
            U256::from(1_010_000_000_000_000_000u128)
        );
        // Amounts below 100 still get at least 1 wei of headroom, so the
        // boundary stays covered when integer division would otherwise
        // round the 1% buffer to 0.
        assert_eq!(
            spardose_amount_with_buffer(U256::from(99u64)),
            U256::from(100u64)
        );
        assert_eq!(spardose_amount_with_buffer(U256::ONE), U256::from(2u64));
        assert_eq!(spardose_amount_with_buffer(U256::ZERO), U256::ONE);
        // Saturates at U256::MAX instead of overflowing.
        assert_eq!(spardose_amount_with_buffer(U256::MAX), U256::MAX);
    }

    #[test]
    fn discards_inaccurate_quotes() {
        // let's use 0.5 as the base case to avoid rounding issues introduced by float
        // conversion
        let low_threshold = big_decimal_to_big_rational(&BigDecimal::from_str("0.5").unwrap());
        let high_threshold = big_decimal_to_big_rational(&BigDecimal::from_str("0.51").unwrap());

        let sell_token = Address::repeat_byte(1);
        let buy_token = Address::repeat_byte(2);

        let query = PriceQuery {
            in_amount: 1_000.try_into().unwrap(),
            kind: OrderKind::Sell,
            sell_token,
            buy_token,
        };

        // buy token is lost
        let tokens_lost = hashmap! {
            sell_token => BigRational::from_integer(500.into()),
        };
        let summary = SettleOutput {
            gas_used: U256::ZERO,
            out_amount: I512::try_from(2_000).unwrap(),
            tokens_lost,
        };
        let estimate =
            ensure_quote_accuracy(&low_threshold, &query, &TradeKind::default(), &summary);
        assert!(matches!(estimate, Err(Error::SimulationFailed(_))));

        // sell token is lost
        let tokens_lost = hashmap! {
            buy_token => BigRational::from_integer(0.into()),
        };
        let summary = SettleOutput {
            gas_used: U256::ZERO,
            out_amount: I512::try_from(2_000).unwrap(),
            tokens_lost,
        };

        let estimate =
            ensure_quote_accuracy(&low_threshold, &query, &TradeKind::default(), &summary);
        assert!(matches!(estimate, Err(Error::SimulationFailed(_))));

        // everything is in-place
        let tokens_lost = hashmap! {
            sell_token => BigRational::from_integer(400.into()),
            buy_token => BigRational::from_integer(0.into()),
        };
        let summary = SettleOutput {
            gas_used: U256::ZERO,
            out_amount: I512::try_from(2_000).unwrap(),
            tokens_lost,
        };
        let estimate =
            ensure_quote_accuracy(&low_threshold, &query, &TradeKind::default(), &summary);
        assert!(estimate.is_ok());

        let tokens_lost = hashmap! {
            sell_token => BigRational::from_integer(500.into()),
            buy_token => BigRational::from_integer(0.into()),
        };

        let sell_more = SettleOutput {
            gas_used: U256::ZERO,
            out_amount: I512::try_from(2_000).unwrap(),
            tokens_lost,
        };

        let estimate =
            ensure_quote_accuracy(&low_threshold, &query, &Default::default(), &sell_more);
        assert!(matches!(estimate, Err(Error::BuffersPayForOrder)));

        // passes with slightly higher tolerance
        let estimate =
            ensure_quote_accuracy(&high_threshold, &query, &Default::default(), &sell_more);
        assert!(estimate.is_ok());

        let tokens_lost = hashmap! {
            sell_token => BigRational::from_integer(0.into()),
            buy_token => BigRational::from_integer(1_000.into()),
        };

        let pay_out_more = SettleOutput {
            gas_used: U256::ZERO,
            out_amount: I512::try_from(2_000).unwrap(),
            tokens_lost,
        };

        let estimate =
            ensure_quote_accuracy(&low_threshold, &query, &Default::default(), &pay_out_more);
        assert!(matches!(estimate, Err(Error::BuffersPayForOrder)));

        // passes with slightly higher tolerance
        let estimate =
            ensure_quote_accuracy(&high_threshold, &query, &Default::default(), &pay_out_more);
        assert!(estimate.is_ok());

        let tokens_lost = hashmap! {
            sell_token => BigRational::from_integer((-500).into()),
            buy_token => BigRational::from_integer(0.into()),
        };

        let sell_less = SettleOutput {
            gas_used: U256::ZERO,
            out_amount: I512::try_from(2_000).unwrap(),
            tokens_lost,
        };
        // Ending up with surplus in the buffers is always fine
        let estimate =
            ensure_quote_accuracy(&low_threshold, &query, &Default::default(), &sell_less);
        assert!(estimate.is_ok());

        let tokens_lost = hashmap! {
            sell_token => BigRational::from_integer(0.into()),
            buy_token => BigRational::from_integer((-1_000).into()),
        };

        let pay_out_less = SettleOutput {
            gas_used: U256::ZERO,
            out_amount: I512::try_from(2_000).unwrap(),
            tokens_lost,
        };
        // Ending up with surplus in the buffers is always fine
        let estimate =
            ensure_quote_accuracy(&low_threshold, &query, &Default::default(), &pay_out_less);
        assert!(estimate.is_ok());
    }
}
