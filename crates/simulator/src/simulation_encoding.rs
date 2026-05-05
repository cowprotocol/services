use {
    crate::{
        encoding::{
            EncodedSettlement,
            Interactions,
            encode_interactions,
            encode_trade,
            encode_wrapper_settlement,
        },
        simulation_builder::{
            AccountOverrideRequest,
            Block,
            BuildError,
            EthCallInputs,
            ExecutionAmount,
            MergeConflict,
            Order,
            PriceEncoding,
            SimulationBuilder,
            Solver,
            WrapperConfig,
        },
    },
    alloy_primitives::{Address, B256, Bytes, U256, keccak256},
    alloy_rpc_types::{
        TransactionRequest,
        state::{AccountOverride, StateOverride},
    },
    alloy_sol_types::SolCall,
    balance_overrides::{BalanceOverrideRequest, BalanceOverriding},
    model::{interaction::InteractionData, order::OrderKind},
};

pub(crate) async fn encode(mut builder: SimulationBuilder) -> Result<EthCallInputs, BuildError> {
    if builder.orders.is_empty() {
        return Err(BuildError::NoOrder);
    }

    let block = match builder.block {
        Block::Latest => builder.simulator.0.current_block.borrow().number,
        Block::Number(n) => n,
    };

    let executed_amounts = futures::future::try_join_all(
        builder
            .orders
            .iter()
            .map(|o| executed_amount(&builder, o, block)),
    )
    .await?;

    // Each order occupies exactly 2 consecutive slots in the token/price
    // vectors: [2*i] = sell_token, [2*i+1] = buy_token.
    // This lets every order be encoded independently without requiring a shared
    // global token list.
    let n = builder.orders.len();
    let mut tokens = Vec::with_capacity(n * 2);
    let mut clearing_prices = Vec::with_capacity(n * 2);
    for order in &builder.orders {
        let (sell_price, buy_price) = match &order.price_encoding {
            PriceEncoding::LimitPrice => (order.data.buy_amount, order.data.sell_amount),
            PriceEncoding::Custom {
                sell_price,
                buy_price,
            } => (*sell_price, *buy_price),
        };
        tokens.push(order.data.sell_token);
        tokens.push(order.data.buy_token);
        clearing_prices.push(sell_price);
        clearing_prices.push(buy_price);
    }

    // Expand any BuyTokensForBuffers request into one Balance override per
    // order, then remove all BuyTokensForBuffers entries so duplicates are
    // impossible.
    if builder
        .account_override_requests
        .iter()
        .any(|r| matches!(r, AccountOverrideRequest::BuyTokensForBuffers))
    {
        builder
            .account_override_requests
            .retain(|r| !matches!(r, AccountOverrideRequest::BuyTokensForBuffers));
        let settlement = *builder.simulator.0.settlement.address();
        for (i, (order, &exec)) in builder.orders.iter().zip(&executed_amounts).enumerate() {
            let sell_price = clearing_prices[2 * i];
            let buy_price = clearing_prices[2 * i + 1];
            let amount = match order.data.kind {
                OrderKind::Sell => sell_price
                    .saturating_mul(exec)
                    .checked_div(buy_price)
                    .unwrap_or(U256::MAX),
                OrderKind::Buy => exec,
            }
            // give 1 wei extra to avoid issues with rounding divisions
            .saturating_add(U256::ONE);
            builder
                .account_override_requests
                .push(AccountOverrideRequest::Balance {
                    holder: settlement,
                    token: order.data.buy_token,
                    amount,
                });
        }
    }

    // Encode every order as a trade, then collect all their interactions.
    let mut trades = Vec::with_capacity(n);
    let mut all_order_pre: Vec<InteractionData> = vec![];
    let mut all_order_post: Vec<InteractionData> = vec![];
    for (i, (order, exec)) in builder.orders.iter().zip(&executed_amounts).enumerate() {
        trades.push(encode_trade(
            &order.data,
            &order.signature,
            order.owner,
            2 * i,
            2 * i + 1,
            *exec,
        ));
        all_order_pre.extend_from_slice(&order.pre_interactions);
        all_order_post.extend_from_slice(&order.post_interactions);
    }

    let settlement = EncodedSettlement {
        tokens,
        clearing_prices,
        trades,
        interactions: Interactions {
            // order pre-hooks run before any additional pre-interactions
            pre: encode_interactions(all_order_pre.iter().chain(&builder.pre_interactions)),
            main: encode_interactions(&builder.main_interactions),
            // additional post-interactions run before order post-hooks
            post: encode_interactions(builder.post_interactions.iter().chain(&all_order_post)),
        },
    };

    let settle_calldata = {
        let mut bytes = settlement.into_settle_call().to_vec();
        if let Some(id) = builder.auction_id {
            bytes.extend_from_slice(&id.to_be_bytes());
        }
        bytes.into()
    };

    let wrapper = builder.wrapper;
    let (to, input) = match wrapper {
        WrapperConfig::Custom(wrappers) if !wrappers.is_empty() => {
            encode_wrapper_settlement(&wrappers, settle_calldata).expect("wrappers is non-empty")
        }
        WrapperConfig::Flashloan(loans) => {
            let calldata = contracts::FlashLoanRouter::FlashLoanRouter::flashLoanAndSettleCall {
                loans: loans
                    .into_iter()
                    .map(|l| contracts::FlashLoanRouter::LoanRequest::Data {
                        amount: l.amount,
                        borrower: l.borrower,
                        lender: l.lender,
                        token: l.token,
                    })
                    .collect(),
                settlement: settle_calldata,
            }
            .abi_encode()
            .into();
            (builder.simulator.0.flash_loan_router, calldata)
        }
        _ => (*builder.simulator.0.settlement.address(), settle_calldata),
    };

    let from = match builder.solver {
        Some(Solver::OriginUnaltered(addr)) => addr,
        Some(Solver::Fake(opt)) => {
            let addr = opt.unwrap_or_else(Address::random);
            builder
                .account_override_requests
                .push(AccountOverrideRequest::SufficientEthBalance(addr));
            builder
                .account_override_requests
                .push(AccountOverrideRequest::AuthenticateAsSolver(addr));
            addr
        }
        None => return Err(BuildError::NoSolver),
    };
    let state_overrides = build_final_state_overrides(
        builder.account_override_requests,
        builder.simulator.0.balance_overrides.as_ref(),
        builder.simulator.0.authenticator,
    )
    .await;

    Ok(EthCallInputs {
        request: TransactionRequest {
            from: Some(from),
            to: Some(to.into()),
            input: input.into(),
            gas: Some(builder.simulator.0.max_gas_limit),
            ..Default::default()
        },
        state_overrides,
        block,
        simulator: builder.simulator,
    })
}

async fn executed_amount(
    builder: &SimulationBuilder,
    order: &Order,
    block: u64,
) -> Result<U256, BuildError> {
    let full = match order.data.kind {
        OrderKind::Sell => order.data.sell_amount,
        OrderKind::Buy => order.data.buy_amount,
    };

    Ok(match order.executed_amount {
        ExecutionAmount::Full => full,
        ExecutionAmount::Explicit(amount) => amount,
        ExecutionAmount::Remaining => {
            let uid = order
                .data
                .uid(&builder.simulator.0.domain_separator, order.owner);
            let filled_amount = builder
                .simulator
                .0
                .settlement
                .filledAmount(Bytes::from(uid.0))
                .block(block.into())
                .call()
                .await
                .map_err(|err| BuildError::FilledAmountQuery(err.into()))?;
            full.saturating_sub(filled_amount)
        }
    })
}

/// Resolves all [`AccountOverrideRequest`]s concurrently on a best-effort
/// basis. Failures are logged and the corresponding override is skipped rather
/// than aborting the whole build.
async fn build_final_state_overrides(
    requests: Vec<AccountOverrideRequest>,
    balance_overrides: &dyn BalanceOverriding,
    authenticator: Address,
) -> StateOverride {
    let futures = requests.into_iter().map(|request| async move {
        match request {
            AccountOverrideRequest::SufficientEthBalance(addr) => Some((
                addr,
                AccountOverride::default().with_balance(U256::MAX / U256::from(2)),
            )),
            AccountOverrideRequest::AuthenticateAsSolver(addr) => {
                // GPv2AllowListAuthentication stores `mapping(address => bool) managers`
                // at storage slot 1. Solidity mapping key: keccak256(address_padded ++
                // slot_padded).
                // <https://github.com/cowprotocol/contracts/blob/main/src/contracts/GPv2AllowListAuthentication.sol#L22>
                let mut buf = [0u8; 64];
                buf[12..32].copy_from_slice(addr.as_slice());
                buf[32..64].copy_from_slice(&U256::ONE.to_be_bytes::<32>());
                let slot = keccak256(buf);
                Some((
                    authenticator,
                    AccountOverride::default()
                        .with_state_diff(std::iter::once((slot, B256::with_last_byte(1)))),
                ))
            }
            AccountOverrideRequest::Balance {
                holder,
                token,
                amount,
            } => {
                let result = balance_overrides
                    .state_override(BalanceOverrideRequest {
                        token,
                        holder,
                        amount,
                    })
                    .await;
                if result.is_none() {
                    tracing::warn!(%token, %holder, "failed to compute balance state override, skipping");
                }
                result
            }
            AccountOverrideRequest::BuyTokensForBuffers => {
                unreachable!(
                    "replaced with specific Balance requests before state overrides get \
                     computed"
                )
            }
            AccountOverrideRequest::Code { account, code } => Some((
                account,
                AccountOverride {
                    code: Some(code),
                    ..Default::default()
                },
            )),
            AccountOverrideRequest::Custom { account, state } => Some((account, state)),
        }
    });

    let mut state_overrides = StateOverride::default();
    for (address, account_override) in futures::future::join_all(futures)
        .await
        .into_iter()
        .flatten()
    {
        if let Err(err) = apply_account_override(&mut state_overrides, address, account_override) {
            tracing::warn!(?err, %address, "conflicting state overrides for address, skipping");
        }
    }
    state_overrides
}

/// Merges `new` into `existing` field by field.
///
/// Returns [`MergeConflict`] if both overrides write the same field.
/// Non-conflicting `state_diff` entries are combined into a single map.
fn merge_account_override(
    existing: &mut AccountOverride,
    new: AccountOverride,
) -> Result<(), MergeConflict> {
    if new.balance.is_some() {
        if existing.balance.is_some() {
            return Err(MergeConflict::Balance);
        }
        existing.balance = new.balance;
    }
    if new.nonce.is_some() {
        if existing.nonce.is_some() {
            return Err(MergeConflict::Nonce);
        }
        existing.nonce = new.nonce;
    }
    if new.code.is_some() {
        if existing.code.is_some() {
            return Err(MergeConflict::Code);
        }
        existing.code = new.code;
    }
    match (new.state, new.state_diff) {
        (Some(new_state), None) => {
            if existing.state.is_some() {
                return Err(MergeConflict::State);
            }
            if existing.state_diff.is_some() {
                return Err(MergeConflict::StateAndStateDiff);
            }
            existing.state = Some(new_state);
        }
        (None, Some(new_diff)) => {
            if existing.state.is_some() {
                return Err(MergeConflict::StateAndStateDiff);
            }
            match &mut existing.state_diff {
                None => existing.state_diff = Some(new_diff),
                Some(existing_diff) => {
                    for (slot, value) in new_diff {
                        if existing_diff.contains_key(&slot) {
                            return Err(MergeConflict::StateDiffSlot(slot));
                        }
                        existing_diff.insert(slot, value);
                    }
                }
            }
        }
        (None, None) => {}
        // alloy does not allow both simultaneously, treat as incompatible
        (Some(_), Some(_)) => return Err(MergeConflict::StateAndStateDiff),
    }
    Ok(())
}

/// Applies `new` to the override map for `address`.
///
/// If `address` already has an entry, the overrides are merged via
/// [`merge_account_override`]. Returns an error on conflict.
fn apply_account_override(
    overrides: &mut StateOverride,
    address: Address,
    new: AccountOverride,
) -> Result<(), MergeConflict> {
    if let Some(existing) = overrides.get_mut(&address) {
        merge_account_override(existing, new)
    } else {
        overrides.insert(address, new);
        Ok(())
    }
}
