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
            Block,
            BuildError,
            EthCallInputs,
            ExecutionAmount,
            Order,
            Prices,
            SimulationBuilder,
            Solver,
            WrapperConfig,
        },
        state_override_helpers::{EthBalanceOverride, SolverAllowlisting},
    },
    alloy_primitives::{Address, Bytes, U256},
    alloy_rpc_types::TransactionRequest,
    alloy_sol_types::SolCall,
    balance_overrides::BalanceOverrideRequest,
    model::order::OrderKind,
};

pub(crate) async fn encode(
    builder: SimulationBuilder,
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

    // Compute before clearing_prices is moved into EncodedSettlement below.
    let fund_amount = builder.fund_settlement_contract.then(|| {
        let base_amount = match order.data.kind {
            OrderKind::Sell => clearing_prices[sell_token_index]
                .saturating_mul(executed_amount)
                .checked_div(clearing_prices[buy_token_index])
                .unwrap_or(U256::MAX),
            OrderKind::Buy => executed_amount,
        };
        // give 1 wei extra to avoid issues with rounding divisions
        base_amount.saturating_add(U256::ONE)
    });

    let trade = encode_trade(
        &order.data,
        &order.signature,
        order.owner,
        sell_token_index,
        buy_token_index,
        executed_amount,
    );

    let order_pre = &order.pre_interactions;
    let order_post = &order.post_interactions;

    let mut settlement = EncodedSettlement {
        tokens,
        clearing_prices,
        trades: vec![trade],
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

    let mut state_overrides = builder.state_overrides;
    let from = match builder.solver {
        Some(Solver::Real(addr)) => addr,
        Some(Solver::Fake(opt)) => {
            let addr = opt.unwrap_or_else(Address::random);
            state_overrides.insert(addr, EthBalanceOverride(U256::MAX / U256::from(2)).into());
            state_overrides.insert(
                builder.simulator.0.authenticator,
                SolverAllowlisting(addr).into(),
            );
            addr
        }
        None => return Err(BuildError::NoSolver),
    };
    if let Some(amount) = fund_amount {
        let (address, state_override) = builder
            .simulator
            .0
            .balance_overrides
            .state_override(BalanceOverrideRequest {
                token: order.data.buy_token,
                holder: *builder.simulator.0.settlement.address(),
                amount,
            })
            .await
            .ok_or(BuildError::FailedToOverrideBalances)?;
        state_overrides.insert(address, state_override);
    }

    for request in builder.fund_requests {
        let (address, account_override) = builder
            .simulator
            .0
            .balance_overrides
            .state_override(request)
            .await
            .ok_or(BuildError::FailedToOverrideBalances)?;
        state_overrides.insert(address, account_override);
    }

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
