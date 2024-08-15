use {
    driver::domain::eth::NonZeroU256,
    e2e::{
        setup::{fee::*, *},
        tx,
        tx_value,
    },
    ethcontract::{prelude::U256, Address},
    model::{
        order::{Order, OrderCreation, OrderCreationAppData, OrderKind},
        quote::{
            OrderQuote,
            OrderQuoteRequest,
            OrderQuoteResponse,
            OrderQuoteSide,
            SellAmount,
            Validity,
        },
        signature::EcdsaSigningScheme,
    },
    reqwest::StatusCode,
    secp256k1::SecretKey,
    serde_json::json,
    shared::ethrpc::Web3,
    web3::signing::SecretKeyRef,
};

#[tokio::test]
#[ignore]
async fn local_node_volume_fee_buy_order() {
    run_test(volume_fee_buy_order_test).await;
}

#[tokio::test]
#[ignore]
async fn local_node_combined_protocol_fees() {
    run_test(combined_protocol_fees).await;
}

async fn combined_protocol_fees(web3: Web3) {
    let limit_surplus_policy = ProtocolFee {
        policy: FeePolicyKind::Surplus {
            factor: 0.3,
            max_volume_factor: 0.9,
        },
        policy_order_class: FeePolicyOrderClass::Limit,
    };
    let market_price_improvement_policy = ProtocolFee {
        policy: FeePolicyKind::PriceImprovement {
            factor: 0.3,
            max_volume_factor: 0.9,
        },
        policy_order_class: FeePolicyOrderClass::Market,
    };
    let partner_fee_app_data = OrderCreationAppData::Full {
        full: json!({
            "version": "1.1.0",
            "metadata": {
                "partnerFee": {
                    "bps":1000,
                    "recipient": "0xb6BAd41ae76A11D10f7b0E664C5007b908bC77C9",
                }
            }
        })
        .to_string(),
    };

    let mut onchain = OnchainComponents::deploy(web3.clone()).await;

    let [solver] = onchain.make_solvers(to_wei(200)).await;
    let [trader] = onchain.make_accounts(to_wei(200)).await;
    let [limit_order_token, market_order_token, partner_fee_order_token] = onchain
        .deploy_tokens_with_weth_uni_v2_pools(to_wei(20), to_wei(20))
        .await;

    for token in &[
        &limit_order_token,
        &market_order_token,
        &partner_fee_order_token,
    ] {
        token.mint(solver.address(), to_wei(1000)).await;
        tx!(
            solver.account(),
            token.approve(
                onchain.contracts().uniswap_v2_router.address(),
                to_wei(1000)
            )
        );
        for trader in &[&trader] {
            tx!(
                trader.account(),
                token.approve(onchain.contracts().uniswap_v2_router.address(), to_wei(100))
            );
        }
    }

    for trader in &[&trader] {
        tx!(
            trader.account(),
            onchain
                .contracts()
                .weth
                .approve(onchain.contracts().allowance, to_wei(100))
        );
        tx_value!(
            trader.account(),
            to_wei(100),
            onchain.contracts().weth.deposit()
        );
    }
    tx!(
        solver.account(),
        onchain
            .contracts()
            .weth
            .approve(onchain.contracts().uniswap_v2_router.address(), to_wei(200))
    );

    let autopilot_config = vec![
        ProtocolFeesConfig(vec![limit_surplus_policy, market_price_improvement_policy]).to_string(),
        "--fee-policy-max-partner-fee=0.02".to_string(),
        "--enable-multiple-fees=true".to_string(),
    ];
    let services = Services::new(onchain.contracts()).await;
    services
        .start_protocol_with_args(
            ExtraServiceArgs {
                autopilot: autopilot_config,
                ..Default::default()
            },
            solver,
        )
        .await;

    tracing::info!("Acquiring quotes.");
    let quote_valid_to = model::time::now_in_epoch_seconds() + 300;
    let sell_amount = to_wei_with_exp(1, 16);
    let [limit_quote_before, market_quote_before, partner_fee_quote] =
        futures::future::try_join_all(
            [
                &limit_order_token,
                &market_order_token,
                &partner_fee_order_token,
            ]
            .map(|token| {
                get_quote(
                    &services,
                    onchain.contracts().weth.address(),
                    token.address(),
                    OrderKind::Sell,
                    sell_amount,
                    quote_valid_to,
                )
            }),
        )
        .await
        .unwrap()
        .try_into()
        .expect("Expected exactly four elements");

    let market_price_improvement_order = OrderCreation {
        sell_amount,
        // to make sure the order is in-market
        buy_amount: market_quote_before.quote.buy_amount * 2 / 3,
        ..sell_order_from_quote(&market_quote_before)
    }
    .sign(
        EcdsaSigningScheme::Eip712,
        &onchain.contracts().domain_separator,
        SecretKeyRef::from(&SecretKey::from_slice(trader.private_key()).unwrap()),
    );
    let limit_surplus_order = OrderCreation {
        sell_amount,
        // to make sure the order is out-of-market
        buy_amount: limit_quote_before.quote.buy_amount * 3 / 2,
        ..sell_order_from_quote(&limit_quote_before)
    }
    .sign(
        EcdsaSigningScheme::Eip712,
        &onchain.contracts().domain_separator,
        SecretKeyRef::from(&SecretKey::from_slice(trader.private_key()).unwrap()),
    );
    let partner_fee_order = OrderCreation {
        sell_amount,
        // to make sure the order is out-of-market
        buy_amount: partner_fee_quote.quote.buy_amount * 3 / 2,
        app_data: partner_fee_app_data.clone(),
        ..sell_order_from_quote(&partner_fee_quote)
    }
    .sign(
        EcdsaSigningScheme::Eip712,
        &onchain.contracts().domain_separator,
        SecretKeyRef::from(&SecretKey::from_slice(trader.private_key()).unwrap()),
    );

    tracing::info!("Rebalancing AMM pools for market & limit order.");
    onchain
        .mint_token_to_weth_uni_v2_pool(&market_order_token, to_wei(1000))
        .await;
    onchain
        .mint_token_to_weth_uni_v2_pool(&limit_order_token, to_wei(1000))
        .await;
    onchain
        .mint_token_to_weth_uni_v2_pool(&partner_fee_order_token, to_wei(1000))
        .await;

    tracing::info!("Waiting for liquidity state to update");
    wait_for_condition(TIMEOUT, || async {
        // Mint blocks until we evict the cached liquidity and fetch the new state.
        onchain.mint_block().await;
        let new_market_order_quote = get_quote(
            &services,
            onchain.contracts().weth.address(),
            market_order_token.address(),
            OrderKind::Sell,
            sell_amount,
            model::time::now_in_epoch_seconds() + 300,
        )
        .await
        .unwrap();
        // Only proceed with test once the quote changes significantly (2x) to avoid
        // progressing due to tiny fluctuations in gas price which would lead to
        // errors down the line.
        new_market_order_quote.quote.buy_amount > market_quote_before.quote.buy_amount * 2
    })
    .await
    .expect("Timeout waiting for eviction of the cached liquidity");

    let [market_quote_after, limit_quote_after, partner_fee_quote_after] =
        futures::future::try_join_all(
            [
                &market_order_token,
                &limit_order_token,
                &partner_fee_order_token,
            ]
            .map(|token| {
                get_quote(
                    &services,
                    onchain.contracts().weth.address(),
                    token.address(),
                    OrderKind::Sell,
                    sell_amount,
                    quote_valid_to,
                )
            }),
        )
        .await
        .unwrap()
        .try_into()
        .expect("Expected exactly two elements");

    let [market_price_improvement_uid, limit_surplus_order_uid, partner_fee_order_uid] =
        futures::future::try_join_all(
            [
                &market_price_improvement_order,
                &limit_surplus_order,
                &partner_fee_order,
            ]
            .map(|order| services.create_order(order)),
        )
        .await
        .unwrap()
        .try_into()
        .expect("Expected exactly four elements");

    tracing::info!("Waiting for orders to trade.");
    let metadata_updated = || async {
        onchain.mint_block().await;
        futures::future::join_all(
            [
                &market_price_improvement_uid,
                &limit_surplus_order_uid,
                &partner_fee_order_uid,
            ]
            .map(|uid| async {
                services
                    .get_order(uid)
                    .await
                    .is_ok_and(|order| !order.metadata.executed_surplus_fee.is_zero())
            }),
        )
        .await
        .into_iter()
        .all(std::convert::identity)
    };
    wait_for_condition(TIMEOUT, metadata_updated)
        .await
        .expect("Timeout waiting for the orders to trade");

    tracing::info!("Checking executions...");
    let market_price_improvement_order = services
        .get_order(&market_price_improvement_uid)
        .await
        .unwrap();
    let market_executed_surplus_fee_in_buy_token =
        surplus_fee_in_buy_token(&market_price_improvement_order, &market_quote_after.quote);
    let market_quote_diff = market_quote_after
        .quote
        .buy_amount
        .saturating_sub(market_quote_before.quote.buy_amount);
    // see `market_price_improvement_policy.factor`, which is 0.3
    assert!(market_executed_surplus_fee_in_buy_token >= market_quote_diff * 3 / 10);

    let partner_fee_order = services.get_order(&partner_fee_order_uid).await.unwrap();
    let partner_fee_executed_surplus_fee_in_buy_token =
        surplus_fee_in_buy_token(&partner_fee_order, &partner_fee_quote_after.quote);
    assert!(
        // see `--fee-policy-max-partner-fee` autopilot config argument, which is 0.02
        partner_fee_executed_surplus_fee_in_buy_token
            >= partner_fee_quote.quote.buy_amount * 2 / 100
    );
    let limit_quote_diff = partner_fee_quote_after
        .quote
        .buy_amount
        .saturating_sub(partner_fee_order.data.buy_amount);
    // see `limit_surplus_policy.factor`, which is 0.3
    assert!(partner_fee_executed_surplus_fee_in_buy_token >= limit_quote_diff * 3 / 10);

    let limit_surplus_order = services.get_order(&limit_surplus_order_uid).await.unwrap();
    let limit_executed_surplus_fee_in_buy_token =
        surplus_fee_in_buy_token(&limit_surplus_order, &limit_quote_after.quote);
    let limit_quote_diff = limit_quote_after
        .quote
        .buy_amount
        .saturating_sub(limit_surplus_order.data.buy_amount);
    // see `limit_surplus_policy.factor`, which is 0.3
    assert!(limit_executed_surplus_fee_in_buy_token >= limit_quote_diff * 3 / 10);
}

async fn get_quote(
    services: &Services<'_>,
    sell_token: Address,
    buy_token: Address,
    kind: OrderKind,
    amount: U256,
    valid_to: u32,
) -> Result<OrderQuoteResponse, (StatusCode, String)> {
    let side = match kind {
        OrderKind::Sell => OrderQuoteSide::Sell {
            sell_amount: SellAmount::BeforeFee {
                value: NonZeroU256::try_from(amount.as_u128()).unwrap(),
            },
        },
        OrderKind::Buy => OrderQuoteSide::Buy {
            buy_amount_after_fee: NonZeroU256::try_from(amount.as_u128()).unwrap(),
        },
    };
    let quote_request = OrderQuoteRequest {
        sell_token,
        buy_token,
        side,
        validity: Validity::To(valid_to),
        ..Default::default()
    };
    services.submit_quote(&quote_request).await
}

fn surplus_fee_in_buy_token(order: &Order, quote: &OrderQuote) -> U256 {
    order.metadata.executed_surplus_fee * quote.buy_amount / quote.sell_amount
}

fn sell_order_from_quote(quote: &OrderQuoteResponse) -> OrderCreation {
    OrderCreation {
        sell_token: quote.quote.sell_token,
        sell_amount: quote.quote.sell_amount,
        buy_token: quote.quote.buy_token,
        buy_amount: quote.quote.buy_amount,
        valid_to: quote.quote.valid_to,
        kind: OrderKind::Sell,
        quote_id: quote.id,
        ..Default::default()
    }
}

async fn volume_fee_buy_order_test(web3: Web3) {
    let fee_policy = FeePolicyKind::Volume { factor: 0.1 };
    let protocol_fee = ProtocolFee {
        policy: fee_policy,
        // The order is in-market, but specifying `Any` order class to make sure it is properly
        // applied
        policy_order_class: FeePolicyOrderClass::Any,
    };
    // Protocol fee set twice to test that only one policy will apply if the
    // autopilot is not configured to support multiple fees
    let protocol_fees_config =
        ProtocolFeesConfig(vec![protocol_fee.clone(), protocol_fee]).to_string();

    let mut onchain = OnchainComponents::deploy(web3.clone()).await;

    let [solver] = onchain.make_solvers(to_wei(1)).await;
    let [trader] = onchain.make_accounts(to_wei(1)).await;
    let [token_gno, token_dai] = onchain
        .deploy_tokens_with_weth_uni_v2_pools(to_wei(1_000), to_wei(1000))
        .await;

    // Fund trader accounts
    token_gno.mint(trader.address(), to_wei(100)).await;

    // Create and fund Uniswap pool
    token_gno.mint(solver.address(), to_wei(1000)).await;
    token_dai.mint(solver.address(), to_wei(1000)).await;
    tx!(
        solver.account(),
        onchain
            .contracts()
            .uniswap_v2_factory
            .create_pair(token_gno.address(), token_dai.address())
    );
    tx!(
        solver.account(),
        token_gno.approve(
            onchain.contracts().uniswap_v2_router.address(),
            to_wei(1000)
        )
    );
    tx!(
        solver.account(),
        token_dai.approve(
            onchain.contracts().uniswap_v2_router.address(),
            to_wei(1000)
        )
    );
    tx!(
        solver.account(),
        onchain.contracts().uniswap_v2_router.add_liquidity(
            token_gno.address(),
            token_dai.address(),
            to_wei(1000),
            to_wei(1000),
            0_u64.into(),
            0_u64.into(),
            solver.address(),
            U256::max_value(),
        )
    );

    // Approve GPv2 for trading
    tx!(
        trader.account(),
        token_gno.approve(onchain.contracts().allowance, to_wei(100))
    );

    // Place Orders
    let services = Services::new(onchain.contracts()).await;
    services
        .start_protocol_with_args(
            ExtraServiceArgs {
                autopilot: vec![protocol_fees_config],
                ..Default::default()
            },
            solver,
        )
        .await;

    let quote = get_quote(
        &services,
        token_gno.address(),
        token_dai.address(),
        OrderKind::Buy,
        to_wei(5),
        model::time::now_in_epoch_seconds() + 300,
    )
    .await
    .unwrap()
    .quote;

    let order = OrderCreation {
        sell_token: token_gno.address(),
        sell_amount: quote.sell_amount * 3 / 2,
        buy_token: token_dai.address(),
        buy_amount: to_wei(5),
        valid_to: model::time::now_in_epoch_seconds() + 300,
        kind: OrderKind::Buy,
        ..Default::default()
    }
    .sign(
        EcdsaSigningScheme::Eip712,
        &onchain.contracts().domain_separator,
        SecretKeyRef::from(&SecretKey::from_slice(trader.private_key()).unwrap()),
    );
    let uid = services.create_order(&order).await.unwrap();

    // Drive solution
    tracing::info!("Waiting for trade.");
    wait_for_condition(TIMEOUT, || async { services.solvable_orders().await == 1 })
        .await
        .unwrap();

    wait_for_condition(TIMEOUT, || async { services.solvable_orders().await == 0 })
        .await
        .unwrap();

    let metadata_updated = || async {
        onchain.mint_block().await;
        let order = services.get_order(&uid).await.unwrap();
        !order.metadata.executed_surplus_fee.is_zero()
    };
    wait_for_condition(TIMEOUT, metadata_updated).await.unwrap();

    let order = services.get_order(&uid).await.unwrap();
    let fee_in_buy_token = quote.fee_amount * quote.buy_amount / quote.sell_amount;
    assert!(order.metadata.executed_surplus_fee >= fee_in_buy_token + quote.sell_amount / 10);

    // Check settlement contract balance
    let balance_after = token_gno
        .balance_of(onchain.contracts().gp_settlement.address())
        .call()
        .await
        .unwrap();
    assert_eq!(order.metadata.executed_surplus_fee, balance_after);
}
