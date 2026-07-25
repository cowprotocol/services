use {
    super::{Estimate, Verification},
    crate::trade_finding::{
        QuoteExecution,
        TradeKind,
        external::dto::{self, Side},
        map_interactions_data,
    },
    ::alloy::sol_types::SolCall,
    alloy::{
        primitives::{Address, Bytes, U256, address, aliases::I512},
        providers::Provider,
        rpc::types::{TransactionRequest, state::StateOverride},
    },
    anyhow::{Context, Result},
    bigdecimal::BigDecimal,
    contracts::support::Solver,
    model::{
        DomainSeparator,
        interaction::InteractionData,
        order::{BuyTokenDestination, OrderData, OrderKind, SellTokenSource},
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
        simulation_builder::{
            self as sim_builder,
            AccountOverrideRequest,
            EthCallInputs,
            ExecutionAmount,
            PriceEncoding,
            SettlementSimulator,
            Solver as SimulationSolver,
        },
        tenderly,
    },
    std::{
        collections::{HashMap, HashSet},
        sync::Arc,
    },
    tracing::instrument,
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
    tenderly: Option<Arc<dyn tenderly::Api>>,
    simulator: SettlementSimulator,
    quote_inaccuracy_limit: BigRational,
    tokens_without_verification: HashSet<Address>,
    min_gas_amount_for_unverified_quotes: u32,
    max_gas_amount_for_unverified_quotes: u32,
}

impl TradeVerifier {
    /// Special contract the simulation setup logic can take sell tokens from in
    /// case the trader doesn't have enough.
    /// We use this **constant** helper contract because computing state
    /// overrides to fake balances for a token can be very expensive. If we
    /// always have to override the balance of the same account we can
    /// compute the necessary state overrides once and cache that.
    /// Additionally using a contract means we can expose a function that
    /// handles token transfers instead of setting up Erc20 approvals.
    /// If you change this value the same constant in `Solver.sol` must be
    /// updated as well.
    const SPARDOSE: Address = address!("0x1111111111111111111111111111111111111111");

    pub fn new(
        simulator: SettlementSimulator,
        tenderly: Option<Arc<dyn tenderly::Api>>,
        quote_inaccuracy_limit: BigDecimal,
        tokens_without_verification: HashSet<Address>,
        min_gas_amount_for_unverified_quotes: u32,
        max_gas_amount_for_unverified_quotes: u32,
    ) -> Self {
        assert!(
            min_gas_amount_for_unverified_quotes <= max_gas_amount_for_unverified_quotes,
            "gas floor ({min_gas_amount_for_unverified_quotes}) exceeds gas ceiling \
             ({max_gas_amount_for_unverified_quotes}) for unverified quotes"
        );
        Self {
            tenderly,
            simulator,
            quote_inaccuracy_limit: big_decimal_to_big_rational(&quote_inaccuracy_limit),
            tokens_without_verification,
            min_gas_amount_for_unverified_quotes,
            max_gas_amount_for_unverified_quotes,
        }
    }

    /// Builds an unverified estimate from the trade's own gas estimate and
    /// promised output amount. Returns an error if the trade provides no gas
    /// estimate, since an unverified quote is meaningless without one.
    fn unverified_estimate(&self, query: &PriceQuery, trade: &TradeKind) -> Result<Estimate> {
        let out_amount = trade
            .out_amount(query)
            .context("failed to compute trade out amount")?;
        trade
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
                    supports_fast_path: trade.supports_fast_path(),
                    execution: QuoteExecution {
                        interactions: map_interactions_data(trade.interactions()),
                        pre_interactions: map_interactions_data(trade.pre_interactions()),
                        jit_orders: trade.jit_orders().cloned().collect(),
                    },
                }
            })
            .context("solver provided no gas estimate")
    }

    /// Executes the simulation and checks quote accuracy.
    async fn get_verified_price_estimate(
        &self,
        swap: &SwapCall,
        query: &PriceQuery,
        verification: &Verification,
        trade: &TradeKind,
    ) -> Result<Estimate, Error> {
        let start = std::time::Instant::now();
        let out_amount = trade.out_amount(query)?;

        let summary = self.execute_and_analyze(swap, query, verification).await?;

        tracing::debug!(
            tokens_lost = ?summary.tokens_lost,
            gas_diff = ?trade.gas_estimate().unwrap_or_default().abs_diff(summary.gas_used.saturating_to()),
            time = ?start.elapsed(),
            promised_out_amount = ?out_amount,
            verified_out_amount = ?summary.out_amount,
            promised_gas = trade.gas_estimate(),
            verified_gas = ?summary.gas_used,
            out_diff = ?(I512::from(out_amount) - summary.out_amount).abs(),
            ?query,
            ?verification,
            "verified quote",
        );

        ensure_quote_accuracy(&self.quote_inaccuracy_limit, query, trade, &summary)
    }

    /// Passes the prepared `settle_call` to the simulation machinery which
    /// makes sure all the pre-conditions are met and collects additional
    /// information throughout the call (e.g. token movements, gas costs).
    fn build_simulation_call(
        &self,
        settle_call: EthCallInputs,
        trade: &TradeKind,
        query: &PriceQuery,
        verification: &Verification,
    ) -> Result<SwapCall, Error> {
        // Use `tx_origin` if response indicates that a special address is needed for
        // the simulation to pass. Otherwise just use the solver address.
        let solver_contract =
            Solver::Instance::new(trade.simulation_solver_address(), self.simulator.provider());

        // `tokens` is passed to `Solver::swap` so it can measure balance changes;
        // it is independent of the settlement's token/price vectors.
        let tokens: Vec<Address> = match trade {
            TradeKind::Legacy(_) => vec![query.sell_token, query.buy_token],
            TradeKind::Regular(trade) => trade.clearing_prices.keys().copied().collect(),
        };

        // compute the sell amount the user needs to have (solver sends the difference
        // between current balance and required balance from the piggy bank to the
        // trader).
        let sell_amount = match query.kind {
            OrderKind::Sell => query.in_amount.get(),
            OrderKind::Buy => trade.out_amount(query)?,
        };

        let call = solver_contract
            .swap(
                self.simulator.settlement_address(),
                tokens.clone(),
                *verification.effective_receiver(),
                verification.from,
                query.sell_token,
                sell_amount,
                settle_call.to,
                settle_call.calldata.clone(),
            )
            .from(*solver_contract.address())
            .gas(self.simulator.max_gas_limit());

        Ok(SwapCall {
            tx_request: call.into_transaction_request(),
            state_overrides: settle_call.state_overrides,
            block: settle_call.block,
            tokens,
            settle_calldata: settle_call.calldata,
        })
    }

    /// Runs the `swap_call` simulation, decodes the returned data, and analyzes
    /// it.
    async fn execute_and_analyze(
        &self,
        swap: &SwapCall,
        query: &PriceQuery,
        verification: &Verification,
    ) -> Result<SettleOutput, Error> {
        let output_bytes = self
            .simulator
            .provider()
            .call(swap.tx_request.clone())
            .overrides(swap.state_overrides.clone())
            .block(swap.block.into())
            .await
            .context("failed to simulate quote")
            .map_err(Error::SimulationFailed)?;

        let output = Solver::Solver::swapCall::abi_decode_returns(&output_bytes)
            .context("failed to decode swap output")
            .map_err(Error::SimulationFailed)?;

        let summary = SettleOutput::from_swap(output, query.kind, &swap.tokens)?
            .with_call_overhead(&swap.settle_calldata);

        Self::post_process_summary(
            self.simulator.settlement_address(),
            summary,
            query,
            verification,
        )
    }

    /// Generates the calldata for the underlying `settle()` call as well as all
    /// the required state overrides (user approval, sell_token balance,
    /// order signature, etc.). This settle call also has a few tweaks that
    /// are specific to the verification aspect:
    /// * has 2 additional interactions tracking relevant balances throughout
    ///   the settlement
    /// * order has a limit price that is trivial to achieve (e.g. buy order
    ///   buys is willing to sell `U256::MAX`, sell order only wants to buy
    ///   `0`). That way the solver can deliver **less** than promised and we
    ///   can still measure how much they actually delivered since the
    ///   simulation will not revert due to limit price violations.
    async fn build_settle_call(
        &self,
        verification: &Verification,
        query: &PriceQuery,
        trade: &TradeKind,
    ) -> Result<EthCallInputs, Error> {
        // assemble all sub-components that are needed to just simulate
        // the proposed trade.
        let override_requests = self.prepare_state_overrides(verification, query, trade)?;
        let fake_order = self.assemble_fake_order(query, verification, trade)?;
        let jit_orders = encode_jit_orders(trade, &self.simulator.domain_separator())?;

        let store_balance = Self::assemble_interaction_for_tracking_balances(
            query,
            verification,
            trade.simulation_solver_address(),
        );

        self
            .simulator
            .new_simulation_builder()
            .with_orders(std::iter::once(fake_order).chain(jit_orders))
            .from_solver(SimulationSolver::OriginUnaltered(trade.simulation_solver_address()))
            .presign_orders()
            .with_overrides(override_requests)
            // the order in which we add interactions is currently quite fragile.
            // Each function appends to the interactions and since we want to measure
            // user balance as the last thing in the pre-interactions and the first thing
            // in the post-interations we first add `store_balance` to the post-interactions
            // then we add the order's interactions with `parameters_from_app_data` and
            // finally we add the `store_balance` pre-interaction.
            .append_post_interactions([store_balance.clone()])
            .parameters_from_app_data(&verification.app_data)?
            .append_pre_interactions(map_interactions_data(trade.pre_interactions()))
            .append_pre_interactions([store_balance])
            .append_main_interactions(map_interactions_data(trade.interactions()))
            .build()
            .await
            .map_err(Error::FailedToBuildSimulation)
    }

    /// Builds an interaction that queries the balances of the relevant account
    /// throughout the settlement to figure out how much they actually get from
    /// the trade.
    /// sell order: track buy token balance of receiver
    /// buy order: track sell token balance of trader
    fn assemble_interaction_for_tracking_balances(
        query: &PriceQuery,
        verification: &Verification,
        solver: Address,
    ) -> InteractionData {
        // storeBalance interactions surrounding the settlement to measure the actual
        // out_amount
        let (tracked_token, tracked_owner) = match query.kind {
            OrderKind::Sell => (query.buy_token, *verification.effective_receiver()),
            OrderKind::Buy => (query.sell_token, verification.from),
        };
        InteractionData {
            target: solver,
            value: U256::ZERO,
            call_data: Solver::Solver::storeBalanceCall {
                token: tracked_token,
                owner: tracked_owner,
                countGas: true,
            }
            .abi_encode(),
        }
    }

    /// There are a few edge cases where the flow of the funds appears to be
    /// nonsensical when they are actually fine. This functions fixes
    /// [`SettleOutput`] for those edge cases:
    /// * trader or receiver is the settlement contrat itself
    /// * sell_token == buy_token
    fn post_process_summary(
        settlement_address: Address,
        mut summary: SettleOutput,
        query: &PriceQuery,
        verification: &Verification,
    ) -> Result<SettleOutput, Error> {
        // Quote accuracy gets determined by how many tokens had to be paid out of the
        // settlement buffers to make the quote happen. When the settlement contract
        // itself is the trader or receiver these values need to be adjusted slightly.
        let (sell_amount, buy_amount) = match query.kind {
            OrderKind::Sell => (I512::from(query.in_amount.get()), summary.out_amount),
            OrderKind::Buy => (summary.out_amount, I512::from(query.in_amount.get())),
        };

        // It looks like the contract lost a lot of sell tokens but only because it was
        // the trader and had to pay for the trade. Adjust tokens lost downward.
        if verification.from == settlement_address {
            summary
                .tokens_lost
                .entry(query.sell_token)
                .and_modify(|balance| *balance -= i512_to_big_rational(&sell_amount));
        }
        // It looks like the contract gained a lot of buy tokens (negative loss) but
        // only because it was the receiver and got the payout. Adjust the tokens lost
        // upward.
        if verification.receiver == settlement_address {
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

        Ok(summary)
    }

    /// Builds a fake order that has the expected sell and buy token but has a
    /// limit price that is trivial to achieve. We do this to avoid reverts
    /// in case the solver promised too much. That way we can still reason
    /// about what the user **would** have gotten from this trade.
    /// As the limit price we use the amounts reported by the quote.
    /// Because a user doesn't provide a signature with their request the
    /// generated order will use pre-sign. A valid signature can trivially
    /// be faked using state overrides.
    fn assemble_fake_order(
        &self,
        query: &PriceQuery,
        verification: &Verification,
        trade: &TradeKind,
    ) -> Result<sim_builder::Order, Error> {
        let out_amount = trade.out_amount(query)?;
        let (sell_price, buy_price) = match trade {
            TradeKind::Legacy(_) => match query.kind {
                OrderKind::Sell => (out_amount, query.in_amount.get()),
                OrderKind::Buy => (query.in_amount.get(), out_amount),
            },
            TradeKind::Regular(trade) => (
                trade
                    .clearing_prices
                    .get(&query.sell_token)
                    .copied()
                    .unwrap_or(U256::ONE),
                trade
                    .clearing_prices
                    .get(&query.buy_token)
                    .copied()
                    .unwrap_or(U256::ONE),
            ),
        };

        let (sell_amount, buy_amount) = match query.kind {
            OrderKind::Sell => (query.in_amount, out_amount),
            OrderKind::Buy => (
                NonZeroU256::try_from(out_amount).context("computed sell amount is zero")?,
                query.in_amount.get(),
            ),
        };

        // Set limit amounts to always pass the settlement check so the actual
        // out_amount can be measured via the storeBalance interactions.
        let (fake_sell_amount, fake_buy_amount) = match query.kind {
            OrderKind::Sell => (sell_amount.get(), U256::ZERO),
            OrderKind::Buy => (sell_amount.get().max(U256::from(u128::MAX)), buy_amount),
        };
        Ok(sim_builder::Order::new(OrderData {
            sell_token: query.sell_token,
            sell_amount: fake_sell_amount,
            buy_token: query.buy_token,
            buy_amount: fake_buy_amount,
            receiver: Some(*verification.effective_receiver()),
            valid_to: u32::MAX,
            app_data: Default::default(),
            fee_amount: U256::ZERO,
            kind: query.kind,
            partially_fillable: false,
            sell_token_balance: SellTokenSource::Erc20,
            buy_token_balance: BuyTokenDestination::Erc20,
        })
        .with_signature(
            verification.from,
            Signature::default_with(SigningScheme::PreSign),
        )
        .fill_at(
            ExecutionAmount::Full,
            PriceEncoding::Custom {
                sell_price,
                buy_price,
            },
        ))
    }

    /// Configures all the state overrides that are needed to mock the given
    /// trade (approval, sell_token balance, helper contract at solver address,
    /// allow listing, ETH balance).
    fn prepare_state_overrides(
        &self,
        verification: &Verification,
        query: &PriceQuery,
        trade: &TradeKind,
    ) -> Result<Vec<AccountOverrideRequest>> {
        let solver = trade.simulation_solver_address();

        // Setup the necessary preconditions (sell token balance, vault relayer
        // approval) using state overrides.
        let needed = match query.kind {
            OrderKind::Sell => query.in_amount.get(),
            OrderKind::Buy => trade.out_amount(query)?,
        };

        let mut requests = vec![
            // give the required approval on behalf of the trader
            AccountOverrideRequest::Approval {
                owner: verification.from,
                spender: self.simulator.vault_relayer_address(),
                amount: needed,
                token: query.sell_token,
            },
            // Deploy the piggy bank account we can draw sell tokens from
            AccountOverrideRequest::Code {
                account: Self::SPARDOSE,
                code: contracts::support::Spardose::Spardose::DEPLOYED_BYTECODE.clone(),
            },
            // Give the piggy bank enough tokens to fund the trader
            AccountOverrideRequest::Balance {
                holder: Self::SPARDOSE,
                token: query.sell_token,
                amount: give_slightly_more(needed),
            },
        ];

        // Set up mocked solver with enough ETH to proceed even if the real account
        // holds none.
        requests.push(AccountOverrideRequest::Code {
            account: solver,
            code: Solver::Solver::DEPLOYED_BYTECODE.clone(),
        });
        // Usually we would require the solver accounts to actually have enough ETH
        // to execute the proposed quote. Otherwise we might get many great quotes
        // which lead to orders that don't get filled because the solver that can
        // settle them actually has no funds.
        // However, this is quite rare and there are also smart contract solvers. Those
        // contracts basically just manage a list of EOAs that are allowed to submit txs
        // on its behalf (similar to our EIP-7702 submission setup). In practice
        // it doesn't make sense for smart contract solvers to hold ETH
        // as they are not the ones paying for the ETH anyway. So in order to avoid
        // teams having to send small amounts of ETH to their contract we fund the
        // solver address with ETH during our simulation.
        requests.push(AccountOverrideRequest::SufficientEthBalance(solver));

        // Some solvers are also market makers and quote via their own inventory
        // - effectively giving out signed orders swapping tokens directly with the
        // user.
        // Due to their security policies some don't want to give out signatures that
        // could actually be used onchain as those would effectively be free options.
        // To still generate verifiable quotes solvers can sign orders that only work
        // for a specific `tx.origin` they are sure nobody actually has control over.
        // For example they would sign an order that can be executed if the tx is
        // executed by account `0x1111..111111`.
        // Since such an address is not a registered solver we register it via state
        // overrides.
        if let Some(custom_origin) = trade.tx_origin()
            && custom_origin != trade.solver()
        {
            requests.push(AccountOverrideRequest::AuthenticateAsSolver(custom_origin))
        }

        Ok(requests)
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
        let unverified_result = self.unverified_estimate(query, &trade);

        let skip_verification = [query.buy_token, query.sell_token]
            .iter()
            .any(|token| self.tokens_without_verification.contains(token));
        if skip_verification {
            tracing::debug!(estimate = ?unverified_result, "quote verification skipped");
            return unverified_result;
        }

        let mut verification = verification.clone();
        // if the user does not have their wallet connected we use a random
        // address because many tokens revert when transfers involve the 0 address.
        if verification.from.is_zero() {
            verification.from = Address::random();
            tracing::debug!(
                trader = ?verification.from,
                "using random trader address with fake balances"
            );
        }

        let swap_call_res = self
            .build_settle_call(&verification, query, &trade)
            .await
            .and_then(|settle_call| {
                self.build_simulation_call(settle_call, &trade, query, &verification)
            });

        let swap_call = match swap_call_res {
            Ok(c) => c,
            Err(err) => {
                tracing::debug!(
                    estimate = ?unverified_result,
                    ?err,
                    "quote verification failed"
                );
                return unverified_result;
            }
        };

        let verification_err = match self
            .get_verified_price_estimate(&swap_call, query, &verification, &trade)
            .await
        {
            Ok(verified) => return Ok(verified),
            Err(err) => err,
        };

        // because this log is extremely large we only emit the resimulation
        // command if the trade had some interactions and it didn't pass verification
        if trade.has_execution_plan()
            && let Some(tenderly) = &self.tenderly
            && let Err(log_err) = tenderly.log_simulation_command(
                swap_call.tx_request,
                swap_call.state_overrides,
                swap_call.block.into(),
            )
        {
            tracing::debug!(?log_err, "could not log tenderly simulation command");
        }

        // For some tokens it's not possible to provide verifiable calldata in the
        // quote (e.g. when they require the use of proprietary APIs which don't give
        // out calldata willy nilly).
        //
        // Since you can't magically make up calldata that makes your quote verifiable
        // solvers don't provide any call data in those cases.
        // This has 2 possible outcomes:
        // 1. the settlement contract has enough buy_tokens to pay for the order =>
        //    Error::BuffersPayForOrder
        // 2. not enough buy tokens in buffer => Error::SimulationFailed
        //
        // To make handling of these quotes more predictable we'll only discard
        // `Error::BuffersPayForOrder` errors if the solver actually tried to provide
        // an execution plan but it's just not correct. In all other cases we just
        // flag the solution as unverified but let it pass.
        if trade.has_execution_plan() && matches!(verification_err, Error::BuffersPayForOrder) {
            tracing::debug!("discarding quote because buffers pay for order");
            Err(verification_err.into())
        } else {
            tracing::debug!(estimate = ?unverified_result, ?verification_err, "quote verification failed");
            unverified_result
        }
    }
}

/// Contains information to execute the simulation request, interpret the result
/// and construct a tenderly simulation from it for debugging.
struct SwapCall {
    tx_request: TransactionRequest,
    state_overrides: StateOverride,
    /// Block number used for both execution and Tenderly logging.
    block: u64,
    /// Token list passed to `Solver::swap` for balance-change tracking.
    tokens: Vec<Address>,
    /// Calldata of the `settle()` call we are analysing. Note that this is
    /// different from the calldata of the `tx_request` as that calls into
    /// additionaly machinery needed for the analysis.
    /// This calldata gets used to finalize the gas used of the simulation.
    settle_calldata: Bytes,
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
        let expected_len = tokens_vec.len() * 2 + 2;
        anyhow::ensure!(
            queriedBalances.len() == expected_len,
            "Solver returned {} balances, expected {} (tokens={})",
            queriedBalances.len(),
            expected_len,
            tokens_vec.len(),
        );

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

    /// Adjusts the reported gas cost since the simulation measurement does not
    /// take into account the 21K units every tx has to pay and the cost for
    /// the calldata. We are overcounting the calldata cost slightly since a
    /// regular settlement would not include the 2 interactions measuring
    /// the token balances.
    fn with_call_overhead(mut self, calldata: &[u8]) -> Self {
        // constants as defined in <https://ethereum.github.io/yellowpaper/paper.pdf>
        // refferred to as Gtransaction, Gtxdatazero, and Gtxdatanonzero
        const TX_BASE_COST: u64 = 21_000;
        const CALLDATA_COST_PER_ZERO_BYTE: u64 = 4;
        const CALLDATA_COST_PER_NONZERO_BYTE: u64 = 16;
        let call_data_cost = calldata
            .iter()
            .map(|byte| {
                if byte == &0x0 {
                    CALLDATA_COST_PER_ZERO_BYTE
                } else {
                    CALLDATA_COST_PER_NONZERO_BYTE
                }
            })
            .sum::<u64>();
        self.gas_used += U256::from(TX_BASE_COST.saturating_add(call_data_cost));
        self
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
        supports_fast_path: trade.supports_fast_path(),
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

fn encode_jit_orders(
    trade: &TradeKind,
    domain_separator: &DomainSeparator,
) -> Result<Vec<sim_builder::Order>> {
    let TradeKind::Regular(trade) = trade else {
        return Ok(vec![]);
    };

    trade
        .jit_orders
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
            Ok(sim_builder::Order::new(order_data)
                .with_signature(owner, signature)
                .fill_at(
                    ExecutionAmount::Explicit(jit_order.executed_amount),
                    PriceEncoding::LimitPrice,
                ))
        })
        .collect::<Result<_>>()
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
    #[error("failed to build the verification simuation")]
    FailedToBuildSimulation(#[from] simulator::simulation_builder::BuildError),
}

/// Some tokens accrue interest over time so the balance on block `n+1`
/// would be bigger than on block `n` with the same state override.
/// To not run into race conditions where we compute the state override
/// on one block but run the actual simulation on the next block we give
/// the user 1% (or at least 1 wei in case of rounding issues) more sell
/// tokens to compensate for that.
fn give_slightly_more(needed: U256) -> U256 {
    let buffer = std::cmp::max(U256::ONE, needed / U256::from(100u64));
    needed.saturating_add(buffer)
}

#[cfg(test)]
mod tests {
    use {super::*, U256, maplit::hashmap, std::str::FromStr};

    #[test]
    fn spardose_amount_applies_1pct_overshoot() {
        assert_eq!(
            give_slightly_more(U256::from(1_000_000_000_000_000_000u128)),
            U256::from(1_010_000_000_000_000_000u128)
        );
        // Amounts below 100 still get at least 1 wei of headroom, so the
        // boundary stays covered when integer division would otherwise
        // round the 1% buffer to 0.
        assert_eq!(give_slightly_more(U256::from(99u64)), U256::from(100u64));
        assert_eq!(give_slightly_more(U256::ONE), U256::from(2u64));
        assert_eq!(give_slightly_more(U256::ZERO), U256::ONE);
        // Saturates at U256::MAX instead of overflowing.
        assert_eq!(give_slightly_more(U256::MAX), U256::MAX);
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

    /// Regression test: when the on-chain `Solver` helper returns fewer
    /// balance entries than expected (observed for wrapper-bearing orders
    /// that revert inside the helper), `from_swap` must return an error
    /// instead of panicking with `index out of bounds`.
    #[test]
    fn from_swap_returns_error_on_short_balances() {
        let tokens_vec = vec![Address::repeat_byte(1), Address::repeat_byte(2)];
        // Expected length is `2 * tokens.len() + 2 = 6`. We feed 4, which is
        // what the verifier observed in production for a wrapper order.
        let swap_return = Solver::Solver::swapReturn {
            gasUsed: U256::ZERO,
            queriedBalances: vec![U256::ZERO; 4],
        };
        let err = SettleOutput::from_swap(swap_return, OrderKind::Sell, &tokens_vec)
            .expect_err("from_swap must surface a short response as an error rather than panic");
        let msg = err.to_string();
        assert!(msg.contains("4"), "error must mention actual length: {msg}");
        assert!(
            msg.contains("6"),
            "error must mention expected length: {msg}"
        );
    }

    #[test]
    fn from_swap_accepts_expected_length() {
        let tokens_vec = vec![Address::repeat_byte(1), Address::repeat_byte(2)];
        let swap_return = Solver::Solver::swapReturn {
            gasUsed: U256::ZERO,
            queriedBalances: vec![U256::ZERO; 6],
        };
        SettleOutput::from_swap(swap_return, OrderKind::Sell, &tokens_vec)
            .expect("well-formed response must parse");
    }

    /// Regression test for same sell==buy token verification, which buy orders
    /// can now reach via `SameTokensPolicy::Allow` (issue #3963). When the
    /// trader is also the receiver the simulated balance only ever shrinks, so
    /// `from_swap` reports the *net* change. The `query.in_amount + out_amount`
    /// correction must recover the gross amount the trader perceives, for both
    /// order kinds.
    #[test]
    fn post_process_same_token_recovers_gross_amount() {
        let token_a = Address::repeat_byte(1);
        let token_b = Address::repeat_byte(3);
        let trader = Address::repeat_byte(2);
        // A settlement address that is neither trader nor receiver, so the
        // buffer adjustments stay untouched and we isolate the same-token fix.
        let settlement = Address::repeat_byte(0xff);
        // Zero receiver means the trader is also the receiver.
        let verification = Verification {
            from: trader,
            receiver: Address::ZERO,
            app_data: Default::default(),
        };
        let summary = |out_amount: I512| SettleOutput {
            gas_used: U256::ZERO,
            out_amount,
            tokens_lost: hashmap! { token_a => BigRational::from_integer(0.into()) },
        };

        // Buy 100 of `token` back into the same account: the trader pays 103
        // gross, so the simulation measures a net change of 103 - 100 = 3. The
        // correction must report the 103 actually sold.
        let buy_query = PriceQuery {
            in_amount: 100.try_into().unwrap(),
            kind: OrderKind::Buy,
            sell_token: token_a,
            buy_token: token_a,
        };
        let out = TradeVerifier::post_process_summary(
            settlement,
            summary(I512::try_from(3).unwrap()),
            &buy_query,
            &verification,
        )
        .unwrap();
        assert_eq!(out.out_amount, I512::try_from(103).unwrap());

        // Sell 100 of `token` back into the same account: the trader nets 98,
        // so the simulation measures 98 - 100 = -2. The correction must report
        // the 98 actually received.
        let sell_query = PriceQuery {
            in_amount: 100.try_into().unwrap(),
            kind: OrderKind::Sell,
            sell_token: token_a,
            buy_token: token_a,
        };
        let out = TradeVerifier::post_process_summary(
            settlement,
            summary(I512::ZERO - I512::try_from(2).unwrap()),
            &sell_query,
            &verification,
        )
        .unwrap();
        assert_eq!(out.out_amount, I512::try_from(98).unwrap());

        // Sanity: with distinct tokens the correction must NOT fire, so a
        // negative out amount is still rejected as the buffers paying the order.
        let distinct_query = PriceQuery {
            in_amount: 100.try_into().unwrap(),
            kind: OrderKind::Buy,
            sell_token: token_a,
            buy_token: token_b,
        };
        let err = TradeVerifier::post_process_summary(
            settlement,
            summary(I512::ZERO - I512::try_from(2).unwrap()),
            &distinct_query,
            &verification,
        );
        assert!(matches!(err, Err(Error::BuffersPayForOrder)));
    }
}
