use {
    ::alloy::primitives::U256 as AlloyU256,
    driver::domain::eth::NonZeroU256,
    e2e::{
        assert_approximately_eq,
        setup::{eth, fee::*, *},
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
async fn local_node_volume_fee_buy_upcoming_future_order() {
    run_test(volume_fee_buy_order_upcoming_future_test).await;
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

#[tokio::test]
#[ignore]
async fn local_node_volume_fee_overrides() {
    run_test(volume_fee_overrides).await;
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

    let [solver] = onchain.make_solvers(eth(200)).await;
    let [trader] = onchain.make_accounts(eth(200)).await;
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
        token.mint(solver.address(), eth(1000)).await;

        token
            .approve(*onchain.contracts().uniswap_v2_router.address(), eth(1000))
            .from(solver.address())
            .send_and_watch()
            .await
            .unwrap();

        token
            .approve(*onchain.contracts().uniswap_v2_router.address(), eth(100))
            .from(trader.address())
            .send_and_watch()
            .await
            .unwrap();
    }

    onchain
        .contracts()
        .weth
        .approve(onchain.contracts().allowance.into_alloy(), eth(100))
        .from(trader.address())
        .send_and_watch()
        .await
        .unwrap();
    onchain
        .contracts()
        .weth
        .deposit()
        .from(trader.address())
        .value(eth(100))
        .send_and_watch()
        .await
        .unwrap();
    onchain
        .contracts()
        .weth
        .approve(*onchain.contracts().uniswap_v2_router.address(), eth(200))
        .from(solver.address())
        .send_and_watch()
        .await
        .unwrap();

    let autopilot_config = [
        ProtocolFeesConfig {
            protocol_fees: vec![limit_surplus_policy, market_price_improvement_policy],
            ..Default::default()
        }
        .into_args(),
        vec!["--fee-policy-max-partner-fee=0.02".to_string()],
    ]
    .concat();
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
                    onchain.contracts().weth.address().into_legacy(),
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
        sell_amount: sell_amount.into_alloy(),
        // to make sure the order is in-market
        buy_amount: market_quote_before.quote.buy_amount * AlloyU256::from(2) / AlloyU256::from(3),
        ..sell_order_from_quote(&market_quote_before)
    }
    .sign(
        EcdsaSigningScheme::Eip712,
        &onchain.contracts().domain_separator,
        SecretKeyRef::from(&SecretKey::from_slice(trader.private_key()).unwrap()),
    );
    let limit_surplus_order = OrderCreation {
        sell_amount: sell_amount.into_alloy(),
        // to make sure the order is out-of-market
        buy_amount: limit_quote_before.quote.buy_amount * AlloyU256::from(3) / AlloyU256::from(2),
        ..sell_order_from_quote(&limit_quote_before)
    }
    .sign(
        EcdsaSigningScheme::Eip712,
        &onchain.contracts().domain_separator,
        SecretKeyRef::from(&SecretKey::from_slice(trader.private_key()).unwrap()),
    );
    let partner_fee_order = OrderCreation {
        sell_amount: sell_amount.into_alloy(),
        // to make sure the order is out-of-market
        buy_amount: (partner_fee_quote.quote.buy_amount * AlloyU256::from(3) / AlloyU256::from(2)),
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
        .mint_token_to_weth_uni_v2_pool(&market_order_token, eth(1000))
        .await;
    onchain
        .mint_token_to_weth_uni_v2_pool(&limit_order_token, eth(1000))
        .await;
    onchain
        .mint_token_to_weth_uni_v2_pool(&partner_fee_order_token, eth(1000))
        .await;

    tracing::info!("Waiting for liquidity state to update");
    wait_for_condition(TIMEOUT, || async {
        // Mint blocks until we evict the cached liquidity and fetch the new state.
        onchain.mint_block().await;
        let new_market_order_quote = get_quote(
            &services,
            onchain.contracts().weth.address().into_legacy(),
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
        new_market_order_quote.quote.buy_amount
            > market_quote_before.quote.buy_amount * AlloyU256::from(2)
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
                onchain.contracts().weth.address().into_legacy(),
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
    assert!(
        market_executed_fee_in_buy_token.into_alloy()
            >= (market_quote_diff * AlloyU256::from(3) / AlloyU256::from(10))
    );

    let partner_fee_order = services.get_order(&partner_fee_order_uid).await.unwrap();
    let partner_fee_executed_fee_in_buy_token =
        fee_in_buy_token(&partner_fee_order, &partner_fee_quote_after.quote);
    assert!(
        // see `--fee-policy-max-partner-fee` autopilot config argument, which is 0.02
        partner_fee_executed_fee_in_buy_token.into_alloy()
            >= (partner_fee_quote.quote.buy_amount * AlloyU256::from(2) / AlloyU256::from(100))
    );
    let limit_quote_diff = partner_fee_quote_after
        .quote
        .buy_amount
        .saturating_sub(partner_fee_order.data.buy_amount);
    // see `limit_surplus_policy.factor`, which is 0.3
    assert!(
        partner_fee_executed_fee_in_buy_token.into_alloy()
            >= (limit_quote_diff * AlloyU256::from(3) / AlloyU256::from(10))
    );

    let limit_surplus_order = services.get_order(&limit_surplus_order_uid).await.unwrap();
    let limit_executed_fee_in_buy_token =
        fee_in_buy_token(&limit_surplus_order, &limit_quote_after.quote);
    let limit_quote_diff = limit_quote_after
        .quote
        .buy_amount
        .saturating_sub(limit_surplus_order.data.buy_amount);
    // see `limit_surplus_policy.factor`, which is 0.3
    assert!(
        limit_executed_fee_in_buy_token.into_alloy()
            >= (limit_quote_diff * AlloyU256::from(3) / AlloyU256::from(10))
    );

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
                .balanceOf(*onchain.contracts().gp_settlement.address())
                .call()
                .await
                .map(IntoLegacy::into_legacy)
        }),
    )
    .await
    .unwrap()
    .try_into()
    .expect("Expected exactly four elements");
    assert_approximately_eq!(
        market_executed_fee_in_buy_token.into_alloy(),
        market_order_token_balance.into_alloy()
    );
    assert_approximately_eq!(
        limit_executed_fee_in_buy_token.into_alloy(),
        limit_order_token_balance.into_alloy()
    );
    assert_approximately_eq!(
        partner_fee_executed_fee_in_buy_token.into_alloy(),
        partner_fee_order_token_balance.into_alloy()
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

    let [solver] = onchain.make_solvers(eth(200)).await;
    let [trader] = onchain.make_accounts(eth(200)).await;
    let [token] = onchain
        .deploy_tokens_with_weth_uni_v2_pools(to_wei(20), to_wei(20))
        .await;

    token.mint(solver.address(), eth(1000)).await;

    token
        .approve(*onchain.contracts().uniswap_v2_router.address(), eth(1000))
        .from(solver.address())
        .send_and_watch()
        .await
        .unwrap();

    token
        .approve(*onchain.contracts().uniswap_v2_router.address(), eth(100))
        .from(trader.address())
        .send_and_watch()
        .await
        .unwrap();
    onchain
        .contracts()
        .weth
        .approve(onchain.contracts().allowance.into_alloy(), eth(100))
        .from(trader.address())
        .send_and_watch()
        .await
        .unwrap();
    onchain
        .contracts()
        .weth
        .deposit()
        .from(trader.address())
        .value(eth(100))
        .send_and_watch()
        .await
        .unwrap();
    onchain
        .contracts()
        .weth
        .approve(*onchain.contracts().uniswap_v2_router.address(), eth(200))
        .from(solver.address())
        .send_and_watch()
        .await
        .unwrap();

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
        sell_amount: eth(10),
        sell_token: *onchain.contracts().weth.address(),
        // just set any low amount since it doesn't matter for this test
        buy_amount: eth(1),
        buy_token: *token.address(),
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
        sell_token: sell_token.into_alloy(),
        buy_token: buy_token.into_alloy(),
        side,
        validity: Validity::To(valid_to),
        ..Default::default()
    };
    services.submit_quote(&quote_request).await
}

fn fee_in_buy_token(order: &Order, quote: &OrderQuote) -> U256 {
    (order.metadata.executed_fee.into_alloy() * quote.buy_amount / quote.sell_amount).into_legacy()
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
    let outdated_fee_policy = FeePolicyKind::Volume { factor: 0.0002 };
    let protocol_fee = ProtocolFee {
        policy: fee_policy,
        // The order is in-market, but specifying `Any` order class to make sure it is properly
        // applied
        policy_order_class: FeePolicyOrderClass::Any,
    };
    let outdated_protocol_fee = ProtocolFee {
        policy: outdated_fee_policy,
        policy_order_class: FeePolicyOrderClass::Any,
    };
    // Protocol fee set twice to test that only one policy will apply if the
    // autopilot is not configured to support multiple fees
    let protocol_fee_args = ProtocolFeesConfig {
        protocol_fees: vec![outdated_protocol_fee.clone(), outdated_protocol_fee],
        upcoming_protocol_fees: Some(UpcomingProtocolFees {
            fee_policies: vec![protocol_fee.clone(), protocol_fee],
            // Set the effective time to 10 minutes ago to make sure the new policy
            // is applied
            effective_from_timestamp: chrono::Utc::now() - chrono::Duration::minutes(10),
        }),
    }
    .into_args();

    let mut onchain = OnchainComponents::deploy(web3.clone()).await;

    let [solver] = onchain.make_solvers(eth(1)).await;
    let [trader] = onchain.make_accounts(eth(1)).await;
    let [token_gno, token_dai] = onchain
        .deploy_tokens_with_weth_uni_v2_pools(to_wei(1_000), to_wei(1000))
        .await;

    // Fund trader accounts
    token_gno.mint(trader.address(), eth(100)).await;

    // Create and fund Uniswap pool
    token_gno.mint(solver.address(), eth(1000)).await;
    token_dai.mint(solver.address(), eth(1000)).await;
    onchain
        .contracts()
        .uniswap_v2_factory
        .createPair(*token_gno.address(), *token_dai.address())
        .from(solver.address())
        .send_and_watch()
        .await
        .unwrap();

    token_gno
        .approve(*onchain.contracts().uniswap_v2_router.address(), eth(1000))
        .from(solver.address())
        .send_and_watch()
        .await
        .unwrap();

    token_dai
        .approve(*onchain.contracts().uniswap_v2_router.address(), eth(1000))
        .from(solver.address())
        .send_and_watch()
        .await
        .unwrap();
    onchain
        .contracts()
        .uniswap_v2_router
        .addLiquidity(
            *token_gno.address(),
            *token_dai.address(),
            eth(1000),
            eth(1000),
            ::alloy::primitives::U256::ZERO,
            ::alloy::primitives::U256::ZERO,
            solver.address(),
            ::alloy::primitives::U256::MAX,
        )
        .from(solver.address())
        .send_and_watch()
        .await
        .unwrap();

    // Approve GPv2 for trading

    token_gno
        .approve(onchain.contracts().allowance.into_alloy(), eth(100))
        .from(trader.address())
        .send_and_watch()
        .await
        .unwrap();

    // Place Orders
    let services = Services::new(&onchain).await;
    services
        .start_protocol_with_args(
            ExtraServiceArgs {
                autopilot: protocol_fee_args,
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
        sell_token: *token_gno.address(),
        sell_amount: (quote.sell_amount * AlloyU256::from(3) / AlloyU256::from(2)),
        buy_token: *token_dai.address(),
        buy_amount: eth(5),
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
    assert!(
        order.metadata.executed_fee.into_alloy()
            >= fee_in_buy_token + (quote.sell_amount / AlloyU256::from(10))
    );

    // Check settlement contract balance
    let balance_after = token_gno
        .balanceOf(*onchain.contracts().gp_settlement.address())
        .call()
        .await
        .unwrap()
        .into_legacy();
    assert_eq!(order.metadata.executed_fee, balance_after);
}

async fn volume_fee_buy_order_upcoming_future_test(web3: Web3) {
    let fee_policy = FeePolicyKind::Volume { factor: 0.1 };
    let future_fee_policy = FeePolicyKind::Volume { factor: 0.0002 };
    let protocol_fee = ProtocolFee {
        policy: fee_policy,
        // The order is in-market, but specifying `Any` order class to make sure it is properly
        // applied
        policy_order_class: FeePolicyOrderClass::Any,
    };
    let future_protocol_fee = ProtocolFee {
        policy: future_fee_policy,
        policy_order_class: FeePolicyOrderClass::Any,
    };
    // Protocol fee set twice to test that only one policy will apply if the
    // autopilot is not configured to support multiple fees
    let protocol_fee_args = ProtocolFeesConfig {
        protocol_fees: vec![protocol_fee.clone(), protocol_fee],
        upcoming_protocol_fees: Some(UpcomingProtocolFees {
            fee_policies: vec![future_protocol_fee.clone(), future_protocol_fee],
            // Set the effective time to far in the future to make sure the new policy
            // is NOT applied
            effective_from_timestamp: chrono::Utc::now() + chrono::Duration::days(1),
        }),
    }
    .into_args();

    let mut onchain = OnchainComponents::deploy(web3.clone()).await;

    let [solver] = onchain.make_solvers(eth(1)).await;
    let [trader] = onchain.make_accounts(eth(1)).await;
    let [token_gno, token_dai] = onchain
        .deploy_tokens_with_weth_uni_v2_pools(to_wei(1_000), to_wei(1000))
        .await;

    // Fund trader accounts
    token_gno.mint(trader.address(), eth(100)).await;

    // Create and fund Uniswap pool
    token_gno.mint(solver.address(), eth(1000)).await;
    token_dai.mint(solver.address(), eth(1000)).await;
    onchain
        .contracts()
        .uniswap_v2_factory
        .createPair(*token_gno.address(), *token_dai.address())
        .from(solver.address())
        .send_and_watch()
        .await
        .unwrap();

    token_gno
        .approve(*onchain.contracts().uniswap_v2_router.address(), eth(1000))
        .from(solver.address())
        .send_and_watch()
        .await
        .unwrap();

    token_dai
        .approve(*onchain.contracts().uniswap_v2_router.address(), eth(1000))
        .from(solver.address())
        .send_and_watch()
        .await
        .unwrap();
    onchain
        .contracts()
        .uniswap_v2_router
        .addLiquidity(
            *token_gno.address(),
            *token_dai.address(),
            eth(1000),
            eth(1000),
            ::alloy::primitives::U256::ZERO,
            ::alloy::primitives::U256::ZERO,
            solver.address(),
            ::alloy::primitives::U256::MAX,
        )
        .from(solver.address())
        .send_and_watch()
        .await
        .unwrap();

    // Approve GPv2 for trading

    token_gno
        .approve(onchain.contracts().allowance.into_alloy(), eth(100))
        .from(trader.address())
        .send_and_watch()
        .await
        .unwrap();

    // Place Orders
    let services = Services::new(&onchain).await;
    services
        .start_protocol_with_args(
            ExtraServiceArgs {
                autopilot: protocol_fee_args,
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
        sell_token: *token_gno.address(),
        sell_amount: (quote.sell_amount * AlloyU256::from(3) / AlloyU256::from(2)),
        buy_token: *token_dai.address(),
        buy_amount: eth(5),
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
    assert!(
        order.metadata.executed_fee.into_alloy()
            >= fee_in_buy_token + (quote.sell_amount / AlloyU256::from(10))
    );

    // Check settlement contract balance
    let balance_after = token_gno
        .balanceOf(*onchain.contracts().gp_settlement.address())
        .call()
        .await
        .unwrap()
        .into_legacy();
    assert_eq!(order.metadata.executed_fee, balance_after);
}

/// Tests that volume fee overrides work correctly for both token pairs and
/// buckets. This test creates multiple stablecoin-like tokens and verifies
/// that:
/// 1. Default volume fee applies to regular trades
/// 2. Token pair overrides take precedence (most specific)
/// 3. Token bucket overrides apply when both tokens are in the bucket
/// 4. Precedence is respected: pair > bucket > default
async fn volume_fee_overrides(web3: Web3) {
    let mut onchain = OnchainComponents::deploy(web3.clone()).await;

    let [solver] = onchain.make_solvers(eth(200)).await;
    let [trader] = onchain.make_accounts(eth(200)).await;

    // Deploy tokens: USDC, DAI, USDT (stablecoins), and WETH (non-stablecoin)
    let [token_usdc, token_dai, token_usdt, token_weth_like] = onchain
        .deploy_tokens_with_weth_uni_v2_pools(to_wei(1000), to_wei(1000))
        .await;

    // Fund solver and trader
    for token in &[&token_usdc, &token_dai, &token_usdt, &token_weth_like] {
        token.mint(solver.address(), eth(10000)).await;
        token.mint(trader.address(), eth(1000)).await;

        token
            .approve(*onchain.contracts().uniswap_v2_router.address(), eth(10000))
            .from(solver.address())
            .send_and_watch()
            .await
            .unwrap();

        token
            .approve(onchain.contracts().allowance.into_alloy(), eth(1000))
            .from(trader.address())
            .send_and_watch()
            .await
            .unwrap();
    }

    // Create liquidity pools for all token pairs
    for (token_a, token_b) in [
        (&token_usdc, &token_dai),
        (&token_usdc, &token_usdt),
        (&token_dai, &token_usdt),
        (&token_usdc, &token_weth_like),
        (&token_dai, &token_weth_like),
    ] {
        onchain
            .contracts()
            .uniswap_v2_factory
            .createPair(*token_a.address(), *token_b.address())
            .from(solver.address())
            .send_and_watch()
            .await
            .unwrap();

        token_a
            .approve(*onchain.contracts().uniswap_v2_router.address(), eth(10000))
            .from(solver.address())
            .send_and_watch()
            .await
            .unwrap();

        token_b
            .approve(*onchain.contracts().uniswap_v2_router.address(), eth(10000))
            .from(solver.address())
            .send_and_watch()
            .await
            .unwrap();

        onchain
            .contracts()
            .uniswap_v2_router
            .addLiquidity(
                *token_a.address(),
                *token_b.address(),
                eth(1000),
                eth(1000),
                ::alloy::primitives::U256::ZERO,
                ::alloy::primitives::U256::ZERO,
                solver.address(),
                ::alloy::primitives::U256::MAX,
            )
            .from(solver.address())
            .send_and_watch()
            .await
            .unwrap();
    }

    // Configure fee policies:
    // - Default volume fee: 1% (0.01)
    // - Bucket 1 (2-token pair): USDC-DAI has 0.05% fee (checked first)
    // - Bucket 2 (stablecoins): USDC, DAI, USDT have 0% fee (checked second)
    let default_volume_fee = ProtocolFee {
        policy: FeePolicyKind::Volume { factor: 0.01 },
        policy_order_class: FeePolicyOrderClass::Any,
    };

    let autopilot_config = [
        ProtocolFeesConfig {
            protocol_fees: vec![default_volume_fee],
            ..Default::default()
        }
        .into_args(),
        vec![
            // Bucket overrides (semicolon-separated, checked in order, first match wins):
            // 1. USDC-DAI pair (2-token bucket) has 0.05% fee
            // 2. All stablecoin-to-stablecoin trades have 0% fee
            {
                let config_str = format!(
                    "--volume-fee-bucket-overrides=0.0005:{},{};0:{},{},{}",
                    token_usdc.address(),
                    token_dai.address(),
                    token_usdc.address(),
                    token_dai.address(),
                    token_usdt.address()
                );
                tracing::info!("Volume fee bucket config: {}", config_str);
                config_str
            },
        ],
    ]
    .concat();

    // Orderbook (API) also needs the same bucket overrides for accurate quote
    // generation
    let api_config = vec![
        format!(
            "--volume-fee-bucket-overrides=0.0005:{},{};0:{},{},{}",
            token_usdc.address(),
            token_dai.address(),
            token_usdc.address(),
            token_dai.address(),
            token_usdt.address()
        ),
        "--volume-fee-factor=0.01".to_string(), // Default 1% volume fee
    ];

    let services = Services::new(&onchain).await;
    services
        .start_protocol_with_args(
            ExtraServiceArgs {
                autopilot: autopilot_config,
                api: api_config,
            },
            solver,
        )
        .await;

    let sell_amount = to_wei(10);
    let quote_valid_to = model::time::now_in_epoch_seconds() + 300;

    // Test Case 1: USDC-DAI should use first bucket (2-token bucket, 0.05%)
    // This matches before the larger 3-token bucket
    tracing::info!("Test Case 1: USDC-DAI bucket override (0.05%)");
    tracing::info!(
        "USDC address: {}, DAI address: {}",
        token_usdc.address(),
        token_dai.address()
    );
    let usdc_dai_quote = get_quote(
        &services,
        token_usdc.address().into_legacy(),
        token_dai.address().into_legacy(),
        OrderKind::Sell,
        sell_amount,
        quote_valid_to,
    )
    .await
    .unwrap();

    let usdc_dai_order = OrderCreation {
        sell_amount: sell_amount.into_alloy(),
        buy_amount: usdc_dai_quote.quote.buy_amount * AlloyU256::from(9) / AlloyU256::from(10),
        ..sell_order_from_quote(&usdc_dai_quote)
    }
    .sign(
        EcdsaSigningScheme::Eip712,
        &onchain.contracts().domain_separator,
        SecretKeyRef::from(&SecretKey::from_slice(trader.private_key()).unwrap()),
    );

    let usdc_dai_uid = services.create_order(&usdc_dai_order).await.unwrap();

    // Test Case 2: DAI-USDT pair should use bucket override (0%)
    // Both tokens are in the stablecoin bucket
    tracing::info!("Test Case 2: DAI-USDT bucket override");
    let dai_usdt_quote = get_quote(
        &services,
        token_dai.address().into_legacy(),
        token_usdt.address().into_legacy(),
        OrderKind::Sell,
        sell_amount,
        quote_valid_to,
    )
    .await
    .unwrap();

    let dai_usdt_order = OrderCreation {
        sell_amount: sell_amount.into_alloy(),
        buy_amount: dai_usdt_quote.quote.buy_amount * AlloyU256::from(9) / AlloyU256::from(10),
        ..sell_order_from_quote(&dai_usdt_quote)
    }
    .sign(
        EcdsaSigningScheme::Eip712,
        &onchain.contracts().domain_separator,
        SecretKeyRef::from(&SecretKey::from_slice(trader.private_key()).unwrap()),
    );

    let dai_usdt_uid = services.create_order(&dai_usdt_order).await.unwrap();

    // Test Case 3: USDC-WETH should use default fee (1%)
    // Only one token is in the stablecoin bucket
    tracing::info!("Test Case 3: USDC-WETH default fee");
    let usdc_weth_quote = get_quote(
        &services,
        token_usdc.address().into_legacy(),
        token_weth_like.address().into_legacy(),
        OrderKind::Sell,
        sell_amount,
        quote_valid_to,
    )
    .await
    .unwrap();

    let usdc_weth_order = OrderCreation {
        sell_amount: sell_amount.into_alloy(),
        buy_amount: usdc_weth_quote.quote.buy_amount * AlloyU256::from(9) / AlloyU256::from(10),
        ..sell_order_from_quote(&usdc_weth_quote)
    }
    .sign(
        EcdsaSigningScheme::Eip712,
        &onchain.contracts().domain_separator,
        SecretKeyRef::from(&SecretKey::from_slice(trader.private_key()).unwrap()),
    );

    let usdc_weth_uid = services.create_order(&usdc_weth_order).await.unwrap();

    onchain.mint_block().await;

    // Wait for all orders to trade
    tracing::info!("Waiting for orders to trade.");
    let metadata_updated = || async {
        onchain.mint_block().await;
        futures::future::join_all(
            [&usdc_dai_uid, &dai_usdt_uid, &usdc_weth_uid].map(|uid| async {
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

    // Verify fees
    tracing::info!("Checking executions...");

    // Get gas price from the node to verify network fee calculation
    let gas_price_output = std::process::Command::new("cast")
        .args(["gas-price", "--rpc-url", "http://localhost:8545"])
        .output()
        .expect("failed to get gas price");
    let gas_price_str = String::from_utf8(gas_price_output.stdout).unwrap();
    let gas_price = gas_price_str
        .trim()
        .parse::<u128>()
        .expect("invalid gas price");
    tracing::info!("Current gas price from node: {}", gas_price);

    // Get all executed fees
    let usdc_dai_order_executed = services.get_order(&usdc_dai_uid).await.unwrap();
    let dai_usdt_order_executed = services.get_order(&dai_usdt_uid).await.unwrap();
    let usdc_weth_order_executed = services.get_order(&usdc_weth_uid).await.unwrap();

    let dai_usdt_fee = dai_usdt_order_executed.metadata.executed_fee.into_alloy();
    let usdc_dai_fee = usdc_dai_order_executed.metadata.executed_fee.into_alloy();
    let usdc_weth_fee = usdc_weth_order_executed.metadata.executed_fee.into_alloy();
    tracing::info!(
        "Executed fees - USDC-DAI: {}, DAI-USDT: {}, USDC-WETH: {}",
        usdc_dai_fee,
        dai_usdt_fee,
        usdc_weth_fee
    );

    // Sanity check: fees should be ordered by protocol fee percentage: 0% < 0.05% <
    // 1%
    assert!(
        dai_usdt_fee < usdc_dai_fee && usdc_dai_fee < usdc_weth_fee,
        "Fees should be ordered: DAI-USDT (0%) < USDC-DAI (0.05%) < USDC-WETH (1%)"
    );

    // Verify quote's fee_amount represents pure gas cost (no volume fee)
    // fee_amount should be: (gas_used * gas_price) / sell_token_price
    let quote_fee = dai_usdt_quote.quote.fee_amount;
    tracing::info!(
        "DAI-USDT quote fee_amount: {} (should only include gas cost, no volume fee)",
        quote_fee
    );

    // Fetch native price for DAI token
    let native_price = services
        .get_native_price(token_dai.address())
        .await
        .unwrap();
    let sell_token_price_f64 = native_price.price;
    tracing::info!(
        "DAI native price (wei per token unit): {}",
        sell_token_price_f64
    );

    // Calculate expected network fee: (gas * gas_price) / sell_token_price
    // Using observed gas from logs: 166391
    let estimated_gas = 166391u128;
    let estimated_fee_in_wei = estimated_gas * gas_price;
    let expected_fee_in_sell_token = (estimated_fee_in_wei as f64 / sell_token_price_f64) as u128;
    tracing::info!(
        "Expected network fee: ({} gas * {} wei/gas) / {} price = {} sell_token units",
        estimated_gas,
        gas_price,
        sell_token_price_f64,
        expected_fee_in_sell_token
    );

    // Verify quote fee_amount matches expected gas cost meaing no volume fee
    // applied
    let expected_network_fee = AlloyU256::from(expected_fee_in_sell_token);
    assert!(
        quote_fee >= expected_network_fee * AlloyU256::from(95) / AlloyU256::from(100)
            && quote_fee <= expected_network_fee * AlloyU256::from(105) / AlloyU256::from(100),
        "Quote fee_amount should match pure gas cost within ±5% (no volume fee). Expected: {}, \
         Got: {}",
        expected_network_fee,
        quote_fee
    );

    // Verify executed fee for 0% protocol order matches the quote
    assert!(
        dai_usdt_fee >= quote_fee * AlloyU256::from(98) / AlloyU256::from(100)
            && dai_usdt_fee <= quote_fee * AlloyU256::from(102) / AlloyU256::from(100),
        "DAI-USDT executed fee (0% protocol) should match quote fee_amount (pure gas cost) within \
         ±2%"
    );

    // executed_fee = network_fee + protocol_fee_in_sell_token
    // DAI-USDT has 0% protocol fee, so it's our baseline (network fee only)
    // We use the fee differences to verify protocol fees are applied correctly

    // Test Case 2: USDC-DAI protocol fee component should be ~0.05% of sell_amount
    // Allow ±2% tolerance for rounding and price conversion
    let usdc_dai_protocol_fee = usdc_dai_fee - expected_network_fee;
    let expected_usdc_dai_protocol =
        sell_amount.into_alloy() * AlloyU256::from(5) / AlloyU256::from(10000); // 0.05%
    tracing::info!(
        "USDC-DAI protocol fee component: {} (expected ~{})",
        usdc_dai_protocol_fee,
        expected_usdc_dai_protocol
    );
    assert!(
        usdc_dai_protocol_fee
            >= expected_usdc_dai_protocol * AlloyU256::from(98) / AlloyU256::from(100)
            && usdc_dai_protocol_fee
                <= expected_usdc_dai_protocol * AlloyU256::from(102) / AlloyU256::from(100),
        "USDC-DAI protocol fee should be within ±2% of 0.05% of sell_amount"
    );

    // Test Case 3: USDC-WETH protocol fee component should be ~1% of sell_amount
    // Allow ±2% tolerance for rounding and price conversion
    let usdc_weth_protocol_fee = usdc_weth_fee - expected_network_fee;
    let expected_usdc_weth_protocol =
        sell_amount.into_alloy() * AlloyU256::from(1) / AlloyU256::from(100); // 1%
    tracing::info!(
        "USDC-WETH protocol fee component: {} (expected ~{})",
        usdc_weth_protocol_fee,
        expected_usdc_weth_protocol
    );
    assert!(
        usdc_weth_protocol_fee
            >= expected_usdc_weth_protocol * AlloyU256::from(98) / AlloyU256::from(100)
            && usdc_weth_protocol_fee
                <= expected_usdc_weth_protocol * AlloyU256::from(102) / AlloyU256::from(100),
        "USDC-WETH protocol fee should be within ±2% of 1% of sell_amount"
    );

    // Test Case 4: Ratio check - USDC-WETH protocol fee should be ~20x USDC-DAI
    // protocol fee (1% / 0.05% = 20)
    let fee_ratio = usdc_weth_protocol_fee / usdc_dai_protocol_fee;
    tracing::info!(
        "Protocol fee ratio (USDC-WETH / USDC-DAI): {}x (expected ~20x)",
        fee_ratio
    );
    assert!(
        fee_ratio >= AlloyU256::from(18) && fee_ratio <= AlloyU256::from(22),
        "Protocol fee ratio should be approximately 20x (1% / 0.05%)"
    );

    tracing::info!("All test cases passed!");
}
