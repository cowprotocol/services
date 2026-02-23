use {
    ::alloy::primitives::{Address, U256},
    autopilot::config::{
        Configuration,
        fee_policy::{
            FeePoliciesConfig,
            FeePolicy as ConfigFeePolicy,
            FeePolicyKind as ConfigFeePolicyKind,
            FeePolicyOrderClass as ConfigFeePolicyOrderClass,
            UpcomingFeePolicies,
        },
        solver::Solver,
    },
    driver::domain::eth::NonZeroU256,
    e2e::{assert_approximately_eq, setup::*},
    ethrpc::alloy::CallBuilderExt,
    model::{
        fee_policy::FeePolicy,
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
    number::units::EthUnit,
    reqwest::StatusCode,
    serde_json::json,
    shared::web3::Web3,
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
    let limit_surplus_policy = ConfigFeePolicy {
        kind: ConfigFeePolicyKind::Surplus {
            factor: 0.3.try_into().unwrap(),
            max_volume_factor: 0.9.try_into().unwrap(),
        },
        order_class: ConfigFeePolicyOrderClass::Limit,
    };
    let market_price_improvement_policy = ConfigFeePolicy {
        kind: ConfigFeePolicyKind::PriceImprovement {
            factor: 0.3.try_into().unwrap(),
            max_volume_factor: 0.9.try_into().unwrap(),
        },
        order_class: ConfigFeePolicyOrderClass::Market,
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

    let [solver] = onchain.make_solvers(200u64.eth()).await;
    let [trader] = onchain.make_accounts(200u64.eth()).await;
    let [
        limit_order_token,
        market_order_token,
        partner_fee_order_token,
    ] = onchain
        .deploy_tokens_with_weth_uni_v2_pools(20u64.eth(), 20u64.eth())
        .await;

    for token in &[
        &limit_order_token,
        &market_order_token,
        &partner_fee_order_token,
    ] {
        token.mint(solver.address(), 1000u64.eth()).await;

        token
            .approve(
                *onchain.contracts().uniswap_v2_router.address(),
                1000u64.eth(),
            )
            .from(solver.address())
            .send_and_watch()
            .await
            .unwrap();

        token
            .approve(
                *onchain.contracts().uniswap_v2_router.address(),
                100u64.eth(),
            )
            .from(trader.address())
            .send_and_watch()
            .await
            .unwrap();
    }

    onchain
        .contracts()
        .weth
        .approve(onchain.contracts().allowance, 100u64.eth())
        .from(trader.address())
        .send_and_watch()
        .await
        .unwrap();
    onchain
        .contracts()
        .weth
        .deposit()
        .from(trader.address())
        .value(100u64.eth())
        .send_and_watch()
        .await
        .unwrap();
    onchain
        .contracts()
        .weth
        .approve(
            *onchain.contracts().uniswap_v2_router.address(),
            200u64.eth(),
        )
        .from(solver.address())
        .send_and_watch()
        .await
        .unwrap();

    let (_config_file, config_arg) = Configuration {
        drivers: vec![Solver::test("test_solver", solver.address())],
        fee_policies: FeePoliciesConfig {
            policies: vec![limit_surplus_policy, market_price_improvement_policy],
            max_partner_fee: 0.02.try_into().unwrap(),
            ..Default::default()
        },
        ..Default::default()
    }
    .to_cli_args();

    let services = Services::new(&onchain).await;
    services
        .start_protocol_with_args(
            ExtraServiceArgs {
                autopilot: vec![config_arg],
                ..Default::default()
            },
            solver,
        )
        .await;

    tracing::info!("Acquiring quotes.");
    let quote_valid_to = model::time::now_in_epoch_seconds() + 300;
    let sell_amount = 10u64.eth();
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
                    *onchain.contracts().weth.address(),
                    *token.address(),
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
        buy_amount: market_quote_before.quote.buy_amount * U256::from(2) / U256::from(3),
        ..sell_order_from_quote(&market_quote_before)
    }
    .sign(
        EcdsaSigningScheme::Eip712,
        &onchain.contracts().domain_separator,
        &trader.signer,
    );
    let limit_surplus_order = OrderCreation {
        sell_amount,
        // to make sure the order is out-of-market
        buy_amount: limit_quote_before.quote.buy_amount * U256::from(3) / U256::from(2),
        ..sell_order_from_quote(&limit_quote_before)
    }
    .sign(
        EcdsaSigningScheme::Eip712,
        &onchain.contracts().domain_separator,
        &trader.signer,
    );
    let partner_fee_order = OrderCreation {
        sell_amount,
        // to make sure the order is out-of-market
        buy_amount: (partner_fee_quote.quote.buy_amount * U256::from(3) / U256::from(2)),
        app_data: partner_fee_app_data.clone(),
        ..sell_order_from_quote(&partner_fee_quote)
    }
    .sign(
        EcdsaSigningScheme::Eip712,
        &onchain.contracts().domain_separator,
        &trader.signer,
    );

    tracing::info!("Rebalancing AMM pools for market & limit order.");
    onchain
        .mint_token_to_weth_uni_v2_pool(&market_order_token, 1000u64.eth())
        .await;
    onchain
        .mint_token_to_weth_uni_v2_pool(&limit_order_token, 1000u64.eth())
        .await;
    onchain
        .mint_token_to_weth_uni_v2_pool(&partner_fee_order_token, 1000u64.eth())
        .await;

    tracing::info!("Waiting for liquidity state to update");
    wait_for_condition(TIMEOUT, || async {
        // Mint blocks until we evict the cached liquidity and fetch the new state.
        onchain.mint_block().await;
        let new_market_order_quote = get_quote(
            &services,
            *onchain.contracts().weth.address(),
            *market_order_token.address(),
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
            > market_quote_before.quote.buy_amount * U256::from(2)
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
                *onchain.contracts().weth.address(),
                *token.address(),
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
        market_executed_fee_in_buy_token >= (market_quote_diff * U256::from(3) / U256::from(10))
    );

    let partner_fee_order = services.get_order(&partner_fee_order_uid).await.unwrap();
    let partner_fee_executed_fee_in_buy_token =
        fee_in_buy_token(&partner_fee_order, &partner_fee_quote_after.quote);
    assert!(
        // see `max-partner-fee` in the `[fee-policies]` autopilot config, which is 0.02
        partner_fee_executed_fee_in_buy_token
            >= (partner_fee_quote.quote.buy_amount * U256::from(2) / U256::from(100))
    );
    let limit_quote_diff = partner_fee_quote_after
        .quote
        .buy_amount
        .saturating_sub(partner_fee_order.data.buy_amount);
    // see `limit_surplus_policy.factor`, which is 0.3
    assert!(
        partner_fee_executed_fee_in_buy_token
            >= (limit_quote_diff * U256::from(3) / U256::from(10))
    );

    let limit_surplus_order = services.get_order(&limit_surplus_order_uid).await.unwrap();
    let limit_executed_fee_in_buy_token =
        fee_in_buy_token(&limit_surplus_order, &limit_quote_after.quote);
    let limit_quote_diff = limit_quote_after
        .quote
        .buy_amount
        .saturating_sub(limit_surplus_order.data.buy_amount);
    // see `limit_surplus_policy.factor`, which is 0.3
    assert!(limit_executed_fee_in_buy_token >= (limit_quote_diff * U256::from(3) / U256::from(10)));

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
/// `max-partner-fee` config which defines how much of an order's volume
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

    let [solver] = onchain.make_solvers(200u64.eth()).await;
    let [trader] = onchain.make_accounts(200u64.eth()).await;
    let [token] = onchain
        .deploy_tokens_with_weth_uni_v2_pools(20u64.eth(), 20u64.eth())
        .await;

    token.mint(solver.address(), 1000u64.eth()).await;

    token
        .approve(
            *onchain.contracts().uniswap_v2_router.address(),
            1000u64.eth(),
        )
        .from(solver.address())
        .send_and_watch()
        .await
        .unwrap();

    token
        .approve(
            *onchain.contracts().uniswap_v2_router.address(),
            100u64.eth(),
        )
        .from(trader.address())
        .send_and_watch()
        .await
        .unwrap();
    onchain
        .contracts()
        .weth
        .approve(onchain.contracts().allowance, 100u64.eth())
        .from(trader.address())
        .send_and_watch()
        .await
        .unwrap();
    onchain
        .contracts()
        .weth
        .deposit()
        .from(trader.address())
        .value(100u64.eth())
        .send_and_watch()
        .await
        .unwrap();
    onchain
        .contracts()
        .weth
        .approve(
            *onchain.contracts().uniswap_v2_router.address(),
            200u64.eth(),
        )
        .from(solver.address())
        .send_and_watch()
        .await
        .unwrap();

    let (_config_file, config_arg) = Configuration {
        drivers: vec![Solver::test("test_solver", solver.address())],
        fee_policies: FeePoliciesConfig {
            max_partner_fee: MAX_PARTNER_VOLUME_FEE.try_into().unwrap(),
            ..Default::default()
        },
        ..Default::default()
    }
    .to_cli_args();

    let services = Services::new(&onchain).await;
    services
        .start_protocol_with_args(
            ExtraServiceArgs {
                autopilot: vec![config_arg],
                ..Default::default()
            },
            solver,
        )
        .await;

    let order = OrderCreation {
        sell_amount: 10u64.eth(),
        sell_token: *onchain.contracts().weth.address(),
        // just set any low amount since it doesn't matter for this test
        buy_amount: 1u64.eth(),
        buy_token: *token.address(),
        app_data: partner_fee_app_data.clone(),
        valid_to: model::time::now_in_epoch_seconds() + 300,
        ..Default::default()
    }
    .sign(
        EcdsaSigningScheme::Eip712,
        &onchain.contracts().domain_separator,
        &trader.signer,
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
                value: NonZeroU256::try_from(amount.to::<u128>()).unwrap(),
            },
        },
        OrderKind::Buy => OrderQuoteSide::Buy {
            buy_amount_after_fee: NonZeroU256::try_from(amount.to::<u128>()).unwrap(),
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
    let fee_policy = ConfigFeePolicy {
        kind: ConfigFeePolicyKind::Volume {
            factor: 0.1.try_into().unwrap(),
        },
        // The order is in-market, but specifying `Any` order class to make sure it is properly
        // applied
        order_class: ConfigFeePolicyOrderClass::Any,
    };
    let outdated_fee_policy = ConfigFeePolicy {
        kind: ConfigFeePolicyKind::Volume {
            factor: 0.0002.try_into().unwrap(),
        },
        order_class: ConfigFeePolicyOrderClass::Any,
    };

    let mut onchain = OnchainComponents::deploy(web3.clone()).await;

    let [solver] = onchain.make_solvers(1u64.eth()).await;
    let [trader] = onchain.make_accounts(1u64.eth()).await;
    let [token_gno, token_dai] = onchain
        .deploy_tokens_with_weth_uni_v2_pools(1_000u64.eth(), 1000u64.eth())
        .await;

    // Fund trader accounts
    token_gno.mint(trader.address(), 100u64.eth()).await;

    // Create and fund Uniswap pool
    token_gno.mint(solver.address(), 1000u64.eth()).await;
    token_dai.mint(solver.address(), 1000u64.eth()).await;
    onchain
        .contracts()
        .uniswap_v2_factory
        .createPair(*token_gno.address(), *token_dai.address())
        .from(solver.address())
        .send_and_watch()
        .await
        .unwrap();

    token_gno
        .approve(
            *onchain.contracts().uniswap_v2_router.address(),
            1000u64.eth(),
        )
        .from(solver.address())
        .send_and_watch()
        .await
        .unwrap();

    token_dai
        .approve(
            *onchain.contracts().uniswap_v2_router.address(),
            1000u64.eth(),
        )
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
            1000u64.eth(),
            1000u64.eth(),
            U256::ZERO,
            U256::ZERO,
            solver.address(),
            U256::MAX,
        )
        .from(solver.address())
        .send_and_watch()
        .await
        .unwrap();

    // Approve GPv2 for trading

    token_gno
        .approve(onchain.contracts().allowance, 100u64.eth())
        .from(trader.address())
        .send_and_watch()
        .await
        .unwrap();

    // Place Orders
    // Protocol fee set twice to test that only one policy will apply if the
    // autopilot is not configured to support multiple fees
    let (_config_file, config_arg) = Configuration {
        drivers: vec![Solver::test("test_solver", solver.address())],
        fee_policies: FeePoliciesConfig {
            policies: vec![outdated_fee_policy.clone(), outdated_fee_policy],
            upcoming_policies: UpcomingFeePolicies {
                policies: vec![fee_policy.clone(), fee_policy],
                // Set the effective time to 10 minutes ago to make sure the new policy
                // is applied
                effective_from_timestamp: Some(chrono::Utc::now() - chrono::Duration::minutes(10)),
            },
            ..Default::default()
        },
        ..Default::default()
    }
    .to_cli_args();

    let services = Services::new(&onchain).await;
    services
        .start_protocol_with_args(
            ExtraServiceArgs {
                autopilot: vec![config_arg],
                ..Default::default()
            },
            solver,
        )
        .await;

    let quote = get_quote(
        &services,
        *token_gno.address(),
        *token_dai.address(),
        OrderKind::Buy,
        5u64.eth(),
        model::time::now_in_epoch_seconds() + 300,
    )
    .await
    .unwrap()
    .quote;

    let order = OrderCreation {
        sell_token: *token_gno.address(),
        sell_amount: (quote.sell_amount * U256::from(3) / U256::from(2)),
        buy_token: *token_dai.address(),
        buy_amount: 5u64.eth(),
        valid_to: model::time::now_in_epoch_seconds() + 300,
        kind: OrderKind::Buy,
        ..Default::default()
    }
    .sign(
        EcdsaSigningScheme::Eip712,
        &onchain.contracts().domain_separator,
        &trader.signer,
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
    assert!(order.metadata.executed_fee >= fee_in_buy_token + (quote.sell_amount / U256::from(10)));

    // Check settlement contract balance
    let balance_after = token_gno
        .balanceOf(*onchain.contracts().gp_settlement.address())
        .call()
        .await
        .unwrap();
    assert_eq!(order.metadata.executed_fee, balance_after);
}

async fn volume_fee_buy_order_upcoming_future_test(web3: Web3) {
    let fee_policy = ConfigFeePolicy {
        kind: ConfigFeePolicyKind::Volume {
            factor: 0.1.try_into().unwrap(),
        },
        // The order is in-market, but specifying `Any` order class to make sure it is properly
        // applied
        order_class: ConfigFeePolicyOrderClass::Any,
    };
    let future_fee_policy = ConfigFeePolicy {
        kind: ConfigFeePolicyKind::Volume {
            factor: 0.0002.try_into().unwrap(),
        },
        order_class: ConfigFeePolicyOrderClass::Any,
    };

    let mut onchain = OnchainComponents::deploy(web3.clone()).await;

    let [solver] = onchain.make_solvers(1u64.eth()).await;
    let [trader] = onchain.make_accounts(1u64.eth()).await;
    let [token_gno, token_dai] = onchain
        .deploy_tokens_with_weth_uni_v2_pools(1_000u64.eth(), 1000u64.eth())
        .await;

    // Fund trader accounts
    token_gno.mint(trader.address(), 100u64.eth()).await;

    // Create and fund Uniswap pool
    token_gno.mint(solver.address(), 1000u64.eth()).await;
    token_dai.mint(solver.address(), 1000u64.eth()).await;
    onchain
        .contracts()
        .uniswap_v2_factory
        .createPair(*token_gno.address(), *token_dai.address())
        .from(solver.address())
        .send_and_watch()
        .await
        .unwrap();

    token_gno
        .approve(
            *onchain.contracts().uniswap_v2_router.address(),
            1000u64.eth(),
        )
        .from(solver.address())
        .send_and_watch()
        .await
        .unwrap();

    token_dai
        .approve(
            *onchain.contracts().uniswap_v2_router.address(),
            1000u64.eth(),
        )
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
            1000u64.eth(),
            1000u64.eth(),
            U256::ZERO,
            U256::ZERO,
            solver.address(),
            U256::MAX,
        )
        .from(solver.address())
        .send_and_watch()
        .await
        .unwrap();

    // Approve GPv2 for trading

    token_gno
        .approve(onchain.contracts().allowance, 100u64.eth())
        .from(trader.address())
        .send_and_watch()
        .await
        .unwrap();

    // Place Orders
    // Protocol fee set twice to test that only one policy will apply if the
    // autopilot is not configured to support multiple fees
    let (_config_file, config_arg) = Configuration {
        drivers: vec![Solver::test("test_solver", solver.address())],
        fee_policies: FeePoliciesConfig {
            policies: vec![fee_policy.clone(), fee_policy],
            upcoming_policies: UpcomingFeePolicies {
                policies: vec![future_fee_policy.clone(), future_fee_policy],
                // Set the effective time to far in the future to make sure the new policy
                // is NOT applied
                effective_from_timestamp: Some(chrono::Utc::now() + chrono::Duration::days(1)),
            },
            ..Default::default()
        },
        ..Default::default()
    }
    .to_cli_args();

    let services = Services::new(&onchain).await;
    services
        .start_protocol_with_args(
            ExtraServiceArgs {
                autopilot: vec![config_arg],
                ..Default::default()
            },
            solver,
        )
        .await;

    let quote = get_quote(
        &services,
        *token_gno.address(),
        *token_dai.address(),
        OrderKind::Buy,
        5u64.eth(),
        model::time::now_in_epoch_seconds() + 300,
    )
    .await
    .unwrap()
    .quote;

    let order = OrderCreation {
        sell_token: *token_gno.address(),
        sell_amount: (quote.sell_amount * U256::from(3) / U256::from(2)),
        buy_token: *token_dai.address(),
        buy_amount: 5u64.eth(),
        valid_to: model::time::now_in_epoch_seconds() + 300,
        kind: OrderKind::Buy,
        ..Default::default()
    }
    .sign(
        EcdsaSigningScheme::Eip712,
        &onchain.contracts().domain_separator,
        &trader.signer,
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
    assert!(order.metadata.executed_fee >= fee_in_buy_token + (quote.sell_amount / U256::from(10)));

    // Check settlement contract balance
    let balance_after = token_gno
        .balanceOf(*onchain.contracts().gp_settlement.address())
        .call()
        .await
        .unwrap();

    assert_eq!(order.metadata.executed_fee, balance_after);
}

/// Volume fees can be overriden by defining "buckets" of tokens that have
/// different vol fees than the default. If an order has both the buy and sell
/// token in a bucket its vol fees are used. We test that:
/// - Earlier buckets take precedence
/// - Token bucket overrides apply when both tokens are in the bucket
async fn volume_fee_overrides(web3: Web3) {
    let mut onchain = OnchainComponents::deploy(web3.clone()).await;

    let [solver] = onchain.make_solvers(200u64.eth()).await;
    let [trader] = onchain.make_accounts(200u64.eth()).await;

    // Deploy tokens: USDC, DAI, USDT (stablecoins), and WETH (non-stablecoin)
    let [token_usdc, token_dai, token_usdt, token_weth] = onchain
        .deploy_tokens_with_weth_uni_v2_pools(1000u64.eth(), 1000u64.eth())
        .await;

    // Fund solver and trader
    for token in &[&token_usdc, &token_dai, &token_usdt, &token_weth] {
        token.mint(solver.address(), 10000u64.eth()).await;
        token.mint(trader.address(), 1000u64.eth()).await;

        token
            .approve(
                *onchain.contracts().uniswap_v2_router.address(),
                10000u64.eth(),
            )
            .from(solver.address())
            .send_and_watch()
            .await
            .unwrap();

        token
            .approve(onchain.contracts().allowance, 1000u64.eth())
            .from(trader.address())
            .send_and_watch()
            .await
            .unwrap();
    }

    // Create liquidity pools for all token pairs
    for (token_a, token_b) in [
        (&token_usdc, &token_dai),
        (&token_dai, &token_usdt),
        (&token_usdc, &token_weth),
    ] {
        onchain
            .contracts()
            .uniswap_v2_factory
            .createPair(*token_a.address(), *token_b.address())
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
                1000u64.eth(),
                1000u64.eth(),
                U256::ZERO,
                U256::ZERO,
                solver.address(),
                U256::MAX,
            )
            .from(solver.address())
            .send_and_watch()
            .await
            .unwrap();
    }

    // Default volume fee: 1% (0.01)
    let default_volume_fee = ConfigFeePolicy {
        kind: ConfigFeePolicyKind::Volume {
            factor: 0.01.try_into().unwrap(),
        },
        order_class: ConfigFeePolicyOrderClass::Any,
    };

    // Bucket overrides (comma-separated, checked in order, first match wins):
    // - 2-token pair: USDC-DAI has 0.05% fee (checked first, has precedence)
    // - Stablecoins: USDC, DAI, USDT have 0% fee (checked second)
    let volume_fee_bucket_config = format!(
        "--volume-fee-bucket-overrides=0.0005:{};{},0:{};{};{}",
        token_usdc.address(),
        token_dai.address(),
        token_usdc.address(),
        token_dai.address(),
        token_usdt.address()
    );

    let (_config_file, config_arg) = Configuration {
        drivers: vec![Solver::test("test_solver", solver.address())],
        fee_policies: FeePoliciesConfig {
            policies: vec![default_volume_fee],
            ..Default::default()
        },
        ..Default::default()
    }
    .to_cli_args();

    // Orderbook (API) also needs the same bucket overrides for accurate quote
    // generation
    let api_config = vec![
        volume_fee_bucket_config.clone(),
        "--volume-fee-factor=0.01".to_string(), // Default 1% volume fee
    ];

    let services = Services::new(&onchain).await;
    services
        .start_protocol_with_args(
            ExtraServiceArgs {
                autopilot: vec![config_arg, volume_fee_bucket_config],
                api: api_config,
            },
            solver,
        )
        .await;

    let sell_amount = 10u64.eth();
    let quote_valid_to = model::time::now_in_epoch_seconds() + 300;

    // Test Case 1: USDC-DAI uses first bucket (2-token bucket, 0.05%)
    let usdc_dai_quote = get_quote(
        &services,
        *token_usdc.address(),
        *token_dai.address(),
        OrderKind::Sell,
        sell_amount,
        quote_valid_to,
    )
    .await
    .unwrap();

    let usdc_dai_order = OrderCreation {
        sell_amount,
        buy_amount: usdc_dai_quote.quote.buy_amount * U256::from(9) / U256::from(10),
        ..sell_order_from_quote(&usdc_dai_quote)
    }
    .sign(
        EcdsaSigningScheme::Eip712,
        &onchain.contracts().domain_separator,
        &trader.signer,
    );

    let usdc_dai_uid = services.create_order(&usdc_dai_order).await.unwrap();

    // Test Case 2: DAI-USDT uses stablecoin bucket (0%)
    let dai_usdt_quote = get_quote(
        &services,
        *token_dai.address(),
        *token_usdt.address(),
        OrderKind::Sell,
        sell_amount,
        quote_valid_to,
    )
    .await
    .unwrap();

    let dai_usdt_order = OrderCreation {
        sell_amount,
        buy_amount: dai_usdt_quote.quote.buy_amount * U256::from(9) / U256::from(10),
        ..sell_order_from_quote(&dai_usdt_quote)
    }
    .sign(
        EcdsaSigningScheme::Eip712,
        &onchain.contracts().domain_separator,
        &trader.signer,
    );

    let dai_usdt_uid = services.create_order(&dai_usdt_order).await.unwrap();

    // Test Case 3: USDC-WETH uses default fee (1%)
    let usdc_weth_quote = get_quote(
        &services,
        *token_usdc.address(),
        *token_weth.address(),
        OrderKind::Sell,
        sell_amount,
        quote_valid_to,
    )
    .await
    .unwrap();

    let usdc_weth_order = OrderCreation {
        sell_amount,
        buy_amount: usdc_weth_quote.quote.buy_amount * U256::from(9) / U256::from(10),
        ..sell_order_from_quote(&usdc_weth_quote)
    }
    .sign(
        EcdsaSigningScheme::Eip712,
        &onchain.contracts().domain_separator,
        &trader.signer,
    );

    let usdc_weth_uid = services.create_order(&usdc_weth_order).await.unwrap();

    onchain.mint_block().await;

    // Wait for all orders to trade
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

    // Verify trades endpoint returns correct executedProtocolFees
    let usdc_dai_trade = &services.get_trades(&usdc_dai_uid).await.unwrap()[0];
    let dai_usdt_trade = &services.get_trades(&dai_usdt_uid).await.unwrap()[0];
    let usdc_weth_trade = &services.get_trades(&usdc_weth_uid).await.unwrap()[0];

    assert_volume_fee(usdc_dai_trade, *token_dai.address(), 0.0005, sell_amount);
    assert_volume_fee(dai_usdt_trade, *token_usdt.address(), 0.0, sell_amount);
    assert_volume_fee(usdc_weth_trade, *token_weth.address(), 0.01, sell_amount);
}

fn assert_volume_fee(
    trade: &model::trade::Trade,
    expected_fee_token: ::alloy::primitives::Address,
    expected_factor: f64,
    sell_amount: U256,
) {
    assert_eq!(
        trade.executed_protocol_fees.len(),
        1,
        "Trade should have exactly one protocol fee"
    );
    let executed_fee = &trade.executed_protocol_fees[0];
    assert_eq!(executed_fee.token, expected_fee_token, "Fee token mismatch");
    match executed_fee.policy {
        FeePolicy::Volume { factor } => {
            assert_eq!(factor, expected_factor, "Volume fee factor mismatch");
        }
        _ => panic!("Expected Volume fee policy, got {:?}", executed_fee.policy),
    }

    let fee_amount = U256::from(executed_fee.amount);
    if expected_factor == 0.0 {
        assert!(fee_amount.is_zero(), "Fee should be zero for 0% factor");
    } else {
        // Integer math for: expected_fee = sell_amount * factor
        let factor_scaled = (expected_factor * 10_000.0) as u64;
        let expected_fee = sell_amount * U256::from(factor_scaled) / U256::from(10_000);

        let lower = expected_fee * U256::from(98) / U256::from(100);
        let upper = expected_fee * U256::from(102) / U256::from(100);
        assert!(
            fee_amount >= lower && fee_amount <= upper,
            "Fee should be ~{expected_fee} (±2%), got {fee_amount}"
        );
    }
}
