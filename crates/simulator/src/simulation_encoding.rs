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
            Prices,
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
    model::order::OrderKind,
    std::sync::Arc,
};

pub(crate) async fn encode(
    mut builder: SimulationBuilder,
    customize: impl FnOnce(&mut EncodedSettlement),
) -> Result<EthCallInputs, BuildError> {
    let order = builder.order.as_ref().ok_or(BuildError::NoOrder)?;

    let block = match builder.block {
        Block::Latest => builder.simulator.0.current_block.borrow().number,
        Block::Number(n) => n,
    };

    let executed_amount = executed_amount(&builder, order, block).await?;

    let (tokens, clearing_prices) = match builder.prices {
        Some(Prices::Explicit {
            tokens,
            clearing_prices,
        }) => (tokens, clearing_prices),
        // At limit price: price[sell_token] = buy_amount, price[buy_token] = sell_amount.
        // This makes sell_amount * price[sell] / price[buy] = buy_amount exactly.
        Some(Prices::Limit) => (
            vec![order.data.sell_token, order.data.buy_token],
            vec![order.data.buy_amount, order.data.sell_amount],
        ),
        None => {
            return Err(BuildError::NoPriceEncoding);
        }
    };

    let sell_token_index = tokens
        .iter()
        .position(|t| *t == order.data.sell_token)
        .ok_or(BuildError::MissingSellToken)?;
    let buy_token_index = tokens
        .iter()
        .position(|t| *t == order.data.buy_token)
        .ok_or(BuildError::MissingBuyToken)?;

    // Replace BuyTokensForBuffers placeholders with concrete Balance requests
    // now that the required amounts are known. Must happen before clearing_prices
    // is moved into EncodedSettlement.
    for request in &mut builder.account_override_requests {
        if matches!(request, AccountOverrideRequest::BuyTokensForBuffers) {
            let amount = match order.data.kind {
                OrderKind::Sell => clearing_prices[sell_token_index]
                    .saturating_mul(executed_amount)
                    .checked_div(clearing_prices[buy_token_index])
                    .unwrap_or(U256::MAX),
                OrderKind::Buy => executed_amount,
            }
            // give 1 wei extra to avoid issues with rounding divisions
            .saturating_add(U256::ONE);

            *request = AccountOverrideRequest::Balance {
                holder: *builder.simulator.0.settlement.address(),
                token: order.data.buy_token,
                amount,
            };
        }
    }

    let trade_data = match order.price_encoding {
        PriceEncoding::Exact => std::borrow::Cow::Borrowed(&order.data),
        PriceEncoding::Disadvantageous => {
            let mut d = order.data.clone();
            match d.kind {
                model::order::OrderKind::Sell => d.buy_amount = U256::ZERO,
                model::order::OrderKind::Buy => {
                    d.sell_amount = d.sell_amount.max(U256::from(u128::MAX))
                }
            }
            std::borrow::Cow::Owned(d)
        }
    };
    let trade = encode_trade(
        &trade_data,
        &order.signature,
        order.owner,
        sell_token_index,
        buy_token_index,
        executed_amount,
    );

    let order_pre = &order.pre_interactions;
    let order_post = &order.post_interactions;

    let mut trades = vec![trade];
    trades.extend(builder.extra_trades);

    let mut settlement = EncodedSettlement {
        tokens,
        clearing_prices,
        trades,
        interactions: Interactions {
            // order's pre-hooks run before any additional pre-interactions
            pre: encode_interactions(order_pre.iter().chain(&builder.pre_interactions)),
            main: encode_interactions(&builder.main_interactions),
            // additional post-interactions run before the order's post-hooks
            post: encode_interactions(builder.post_interactions.iter().chain(order_post)),
        },
    };

    customize(&mut settlement);

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
        Some(Solver::Real(addr)) => addr,
        Some(Solver::Fake(opt)) => {
            let addr = opt.unwrap_or_else(Address::random);
            builder
                .account_override_requests
                .push(AccountOverrideRequest::SufficientEthBalance(addr));
            builder
                .account_override_requests
                .push(AccountOverrideRequest::AuthenticateAddress(addr));
            addr
        }
        None => return Err(BuildError::NoSolver),
    };
    let state_overrides = build_final_state_overrides(
        builder.account_override_requests,
        Arc::clone(&builder.simulator.0.balance_overrides),
        builder.simulator.0.authenticator,
    )
    .await?;

    Ok(EthCallInputs {
        request: TransactionRequest {
            from: Some(from),
            to: Some(to.into()),
            input: input.into(),
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

/// Resolves all [`AccountOverrideRequest`]s concurrently, merges them
/// and returns the final [`StateOverride`].
async fn build_final_state_overrides(
    requests: Vec<AccountOverrideRequest>,
    balance_overrides: Arc<dyn BalanceOverriding>,
    authenticator: Address,
) -> Result<StateOverride, BuildError> {
    let futures = requests.into_iter().map(|request| {
        let balance_overrides = Arc::clone(&balance_overrides);
        async move {
            match request {
                AccountOverrideRequest::SufficientEthBalance(addr) => Ok((
                    addr,
                    AccountOverride::default().with_balance(U256::MAX / U256::from(2)),
                )),
                AccountOverrideRequest::AuthenticateAddress(addr) => {
                    // GPv2AllowListAuthentication stores `mapping(address => bool) managers`
                    // at storage slot 1. Solidity mapping key: keccak256(address_padded ++
                    // slot_padded).
                    let mut buf = [0u8; 64];
                    buf[12..32].copy_from_slice(addr.as_slice());
                    buf[32..64].copy_from_slice(&U256::ONE.to_be_bytes::<32>());
                    let slot = keccak256(buf);
                    Ok((
                        authenticator,
                        AccountOverride::default()
                            .with_state_diff(std::iter::once((slot, B256::with_last_byte(1)))),
                    ))
                }
                AccountOverrideRequest::Balance {
                    holder,
                    token,
                    amount,
                } => balance_overrides
                    .state_override(BalanceOverrideRequest {
                        token,
                        holder,
                        amount,
                    })
                    .await
                    .ok_or(BuildError::FailedToOverrideBalances),
                AccountOverrideRequest::BuyTokensForBuffers => {
                    unreachable!(
                        "replaced with specific Balance requests before state overrides get \
                         computed"
                    )
                }
                AccountOverrideRequest::Code { account, code } => Ok((
                    account,
                    AccountOverride {
                        code: Some(code),
                        ..Default::default()
                    },
                )),
                AccountOverrideRequest::Custom { account, state } => Ok((account, state)),
            }
        }
    });
    let resolved_overrides = futures::future::try_join_all(futures).await?;

    let mut state_overrides = StateOverride::default();
    for (address, account_override) in resolved_overrides {
        apply_account_override(&mut state_overrides, address, account_override)
            .map_err(BuildError::ConflictingStateOverrides)?;
    }
    Ok(state_overrides)
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
pub fn apply_account_override(
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
