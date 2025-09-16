use {
    driver::domain::eth::NonZeroU256,
    e2e::{
        assert_approximately_eq,
        setup::{eth, fee::*, *},
        tx,
        tx_value,
    },
    ethcontract::{Address, prelude::U256},
    ethrpc::alloy::{
        CallBuilderExt,
        conversions::{IntoAlloy, IntoLegacy},
    },
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

#[tokio::test]
#[ignore]
async fn local_node_surplus_partner_fee() {
    run_test(surplus_partner_fee).await;
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
    let [
        limit_order_token,
        market_order_token,
        partner_fee_order_token,
    ] = onchain
        .deploy_tokens_with_weth_uni_v2_pools(to_wei(20), to_wei(20))
        .await;

    for token in &[
        &limit_order_token,
        &market_order_token,
        &partner_fee_order_token,
    ] {
        token.mint(solver.address(), to_wei(1000)).await;

        token
            .approve(
                onchain.contracts().uniswap_v2_router.address().into_alloy(),
                eth(1000),
            )
            .from(solver.address().into_alloy())
            .send_and_watch()
            .await
            .unwrap();

        token
            .approve(
                onchain.contracts().uniswap_v2_router.address().into_alloy(),
                eth(100),
            )
            .from(trader.address().into_alloy())
            .send_and_watch()
            .await
            .unwrap();
    }

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
    ];
    let services = Services::new(&onchain).await;
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
    let sell_amount = to_wei(10);
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
                    token.address().into_legacy(),
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
            market_order_token.address().into_legacy(),
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

    let [
        market_quote_after,
        limit_quote_after,
        partner_fee_quote_after,
    ] = futures::future::try_join_all(
        [
            &market_order_token,
            &limit_order_token,
            &partner_fee_order_token,
        ]
        .map(|token| {
            get_quote(
                &services,
                onchain.contracts().weth.address(),
                token.address().into_legacy(),
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

    let [
        market_price_improvement_uid,
        limit_surplus_order_uid,
        partner_fee_order_uid,
    ] = futures::future::try_join_all(
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

    onchain.mint_block().await;

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
                    .is_ok_and(|order| !order.metadata.executed_fee.is_zero())
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
    let market_executed_fee_in_buy_token =
        fee_in_buy_token(&market_price_improvement_order, &market_quote_after.quote);
    let market_quote_diff = market_quote_after
        .quote
        .buy_amount
        .saturating_sub(market_quote_before.quote.buy_amount);
    // see `market_price_improvement_policy.factor`, which is 0.3
    assert!(market_executed_fee_in_buy_token >= market_quote_diff * 3 / 10);

    let partner_fee_order = services.get_order(&partner_fee_order_uid).await.unwrap();
    let partner_fee_executed_fee_in_buy_token =
        fee_in_buy_token(&partner_fee_order, &partner_fee_quote_after.quote);
    assert!(
        // see `--fee-policy-max-partner-fee` autopilot config argument, which is 0.02
        partner_fee_executed_fee_in_buy_token >= partner_fee_quote.quote.buy_amount * 2 / 100
    );
    let limit_quote_diff = partner_fee_quote_after
        .quote
        .buy_amount
        .saturating_sub(partner_fee_order.data.buy_amount);
    // see `limit_surplus_policy.factor`, which is 0.3
    assert!(partner_fee_executed_fee_in_buy_token >= limit_quote_diff * 3 / 10);

    let limit_surplus_order = services.get_order(&limit_surplus_order_uid).await.unwrap();
    let limit_executed_fee_in_buy_token =
        fee_in_buy_token(&limit_surplus_order, &limit_quote_after.quote);
    let limit_quote_diff = limit_quote_after
        .quote
        .buy_amount
        .saturating_sub(limit_surplus_order.data.buy_amount);
    // see `limit_surplus_policy.factor`, which is 0.3
    assert!(limit_executed_fee_in_buy_token >= limit_quote_diff * 3 / 10);

    let [
        market_order_token_balance,
        limit_order_token_balance,
        partner_fee_order_token_balance,
    ] = futures::future::try_join_all(
        [
            &market_order_token,
            &limit_order_token,
            &partner_fee_order_token,
        ]
        .map(|token| async {
            token
                .balanceOf(onchain.contracts().gp_settlement.address().into_alloy())
                .call()
                .await
                .map(|balance| balance.into_legacy())
        }),
    )
    .await
    .unwrap()
    .try_into()
    .expect("Expected exactly four elements");
    assert_approximately_eq!(market_executed_fee_in_buy_token, market_order_token_balance);
    assert_approximately_eq!(limit_executed_fee_in_buy_token, limit_order_token_balance);
    assert_approximately_eq!(
        partner_fee_executed_fee_in_buy_token,
        partner_fee_order_token_balance
    );
}

/// Tests that a partner can provide multiple partner fees and also use
/// the `Surplus` and `PriceImprovement` fee policies. Also checks that
/// the partner fees can not exceed the globally defined
/// `--fee-policy-max-partner-fee` which defines how much of an order's volume
/// may be captured in total by partner fees.
async fn surplus_partner_fee(web3: Web3) {
    // All these values are unreasonably high but result in easier math
    // when it comes to limiting partner fees to the global volume cap.
    const MAX_PARTNER_VOLUME_FEE: f64 = 0.375;
    let partner_fee_app_data = OrderCreationAppData::Full {
        full: json!({
            "version": "1.1.0",
            "metadata": {
                "partnerFee": [
                    // this will use the entire `maxVolumeBps`
                    {
                        "surplusBps": 3_000, // 30%
                        "maxVolumeBps": 2_500, // 25%
                        "recipient": "0xb6BAd41ae76A11D10f7b0E664C5007b908bC77C9",
                    },
                    // this will have the `maxVolumeBps` reduced (to stay below the cap)
                    {
                        "priceImprovementBps": 3_000, // 30%
                        "maxVolumeBps": 1_500, // 15%
                        "recipient": "0xb6BAd41ae76A11D10f7b0E664C5007b908bC77C9",
                    },
                    // this will have the `maxVolumeBps` set to 0 (prev policies already reach the
                    // global cap)
                    {
                        "priceImprovementBps": 3_000, // 30%
                        "maxVolumeBps": 1_500, // 15%
                        "recipient": "0xb6BAd41ae76A11D10f7b0E664C5007b908bC77C9",
                    },
                ]
            }
        })
        .to_string(),
    };

    let mut onchain = OnchainComponents::deploy(web3.clone()).await;

    let [solver] = onchain.make_solvers(to_wei(200)).await;
    let [trader] = onchain.make_accounts(to_wei(200)).await;
    let [token] = onchain
        .deploy_tokens_with_weth_uni_v2_pools(to_wei(20), to_wei(20))
        .await;

    token.mint(solver.address(), to_wei(1000)).await;

    token
        .approve(
            onchain.contracts().uniswap_v2_router.address().into_alloy(),
            eth(1000),
        )
        .from(solver.address().into_alloy())
        .send_and_watch()
        .await
        .unwrap();

    token
        .approve(
            onchain.contracts().uniswap_v2_router.address().into_alloy(),
            eth(100),
        )
        .from(trader.address().into_alloy())
        .send_and_watch()
        .await
        .unwrap();
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
    tx!(
        solver.account(),
        onchain
            .contracts()
            .weth
            .approve(onchain.contracts().uniswap_v2_router.address(), to_wei(200))
    );

    let services = Services::new(&onchain).await;
    services
        .start_protocol_with_args(
            ExtraServiceArgs {
                autopilot: vec![format!(
                    "--fee-policy-max-partner-fee={MAX_PARTNER_VOLUME_FEE}"
                )],
                ..Default::default()
            },
            solver,
        )
        .await;

    let order = OrderCreation {
        sell_amount: to_wei(10),
        sell_token: onchain.contracts().weth.address(),
        // just set any low amount since it doesn't matter for this test
        buy_amount: to_wei(1),
        buy_token: token.address().into_legacy(),
        app_data: partner_fee_app_data.clone(),
        valid_to: model::time::now_in_epoch_seconds() + 300,
        ..Default::default()
    }
    .sign(
        EcdsaSigningScheme::Eip712,
        &onchain.contracts().domain_separator,
        SecretKeyRef::from(&SecretKey::from_slice(trader.private_key()).unwrap()),
    );

    let order_uid = services.create_order(&order).await.unwrap();

    onchain.mint_block().await;

    tracing::info!("Waiting for orders to trade.");
    let metadata_updated = || async {
        onchain.mint_block().await;
        services
            .get_order(&order_uid)
            .await
            .is_ok_and(|order| !order.metadata.executed_fee.is_zero())
    };
    wait_for_condition(TIMEOUT, metadata_updated)
        .await
        .expect("Timeout waiting for the orders to trade");

    tracing::info!("Checking executions...");
    let trades = services.get_trades(&order_uid).await.unwrap();
    assert_eq!(trades.len(), 1);
    let trade = &trades[0];

    assert_eq!(trade.executed_protocol_fees.len(), 3);

    // Fee policies defined by the partner got applied for the
    // executed trades.
    assert_eq!(
        trade.executed_protocol_fees[0].policy,
        model::fee_policy::FeePolicy::Surplus {
            factor: 0.3,
            max_volume_factor: 0.25,
        }
    );

    // Fee policies exceeding the global partner fee cap have been
    // capped to the maximum allowed value.
    assert!(matches!(
        trade.executed_protocol_fees[1].policy,
        model::fee_policy::FeePolicy::PriceImprovement {
            factor: 0.3,
            // Note that the partner fee policy actually specified
            // 0.15 here but that would exceed the total partner fee
            // so it was capped to 0.1.
            max_volume_factor: 0.1,
            .. // we don't care about the quote here
        }
    ));

    // Fee policies exceeding the global partner fee cap have been
    // capped to the maximum allowed value.
    assert!(matches!(
        trade.executed_protocol_fees[2].policy,
        model::fee_policy::FeePolicy::PriceImprovement {
            factor: 0.3,
            // Note that the partner fee policy actually specified
            // 0.15 here but since we already reached the cap the final
            // fee policy is not allowed to capture any more fees.
            max_volume_factor: 0.,
            .. // we don't care about the quote here
        }
    ));

    // The volume caps of all partner fees combined (applied after each other)
    // are capped to the total volume cap for all partners. Note that we use
    // "1. + factor" here because the factors start from 0 (e.g. 20% == 0.2)
    // but the math only works with factors starting from 1 (e.g. 20% == 1.2).
    assert_eq!(
        trade.executed_protocol_fees.iter().fold(1., |acc, fee| {
            acc * (1. + fee.policy.max_volume_factor())
        }),
        1. + MAX_PARTNER_VOLUME_FEE
    );
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

fn fee_in_buy_token(order: &Order, quote: &OrderQuote) -> U256 {
    order.metadata.executed_fee * quote.buy_amount / quote.sell_amount
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
        onchain.contracts().uniswap_v2_factory.create_pair(
            token_gno.address().into_legacy(),
            token_dai.address().into_legacy()
        )
    );

    token_gno
        .approve(
            onchain.contracts().uniswap_v2_router.address().into_alloy(),
            eth(1000),
        )
        .from(solver.address().into_alloy())
        .send_and_watch()
        .await
        .unwrap();

    token_dai
        .approve(
            onchain.contracts().uniswap_v2_router.address().into_alloy(),
            eth(1000),
        )
        .from(solver.address().into_alloy())
        .send_and_watch()
        .await
        .unwrap();
    tx!(
        solver.account(),
        onchain.contracts().uniswap_v2_router.add_liquidity(
            token_gno.address().into_legacy(),
            token_dai.address().into_legacy(),
            to_wei(1000),
            to_wei(1000),
            0_u64.into(),
            0_u64.into(),
            solver.address(),
            U256::max_value(),
        )
    );

    // Approve GPv2 for trading

    token_gno
        .approve(onchain.contracts().allowance.into_alloy(), eth(100))
        .from(trader.address().into_alloy())
        .send_and_watch()
        .await
        .unwrap();

    // Place Orders
    let services = Services::new(&onchain).await;
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
        token_gno.address().into_legacy(),
        token_dai.address().into_legacy(),
        OrderKind::Buy,
        to_wei(5),
        model::time::now_in_epoch_seconds() + 300,
    )
    .await
    .unwrap()
    .quote;

    let order = OrderCreation {
        sell_token: token_gno.address().into_legacy(),
        sell_amount: quote.sell_amount * 3 / 2,
        buy_token: token_dai.address().into_legacy(),
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
    let metadata_updated = || async {
        onchain.mint_block().await;
        let order = services.get_order(&uid).await.unwrap();
        !order.metadata.executed_fee.is_zero()
    };
    wait_for_condition(TIMEOUT, metadata_updated).await.unwrap();

    let order = services.get_order(&uid).await.unwrap();
    let fee_in_buy_token = quote.fee_amount * quote.buy_amount / quote.sell_amount;
    assert!(order.metadata.executed_fee >= fee_in_buy_token + quote.sell_amount / 10);

    // Check settlement contract balance
    let balance_after = token_gno
        .balanceOf(onchain.contracts().gp_settlement.address().into_alloy())
        .call()
        .await
        .unwrap()
        .into_legacy();
    assert_eq!(order.metadata.executed_fee, balance_after);
}
