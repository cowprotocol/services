use {
    driver::domain::eth::NonZeroU256,
    e2e::{
        assert_approximately_eq,
        setup::{colocation::SolverEngine, *},
        tx,
        tx_value,
    },
    ethcontract::{prelude::U256, Address},
    model::{
        order::{OrderCreation, OrderCreationAppData, OrderKind},
        quote::{OrderQuoteRequest, OrderQuoteResponse, OrderQuoteSide, SellAmount, Validity},
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

    let autopilot_config = vec![
        ProtocolFeesConfig(vec![limit_surplus_policy, market_price_improvement_policy]).to_string(),
        "--fee-policy-max-partner-fee=0.02".to_string(),
    ];

    let mut onchain = OnchainComponents::deploy(web3.clone()).await;

    let [solver] = onchain.make_solvers(to_wei(200)).await;
    let [trader] = onchain.make_accounts(to_wei(200)).await;
    let [limit_order_token, market_order_token, partner_fee_order_token] = onchain
        .deploy_tokens_with_weth_uni_v2_pools(to_wei(20), to_wei(20))
        .await;

    limit_order_token.mint(solver.address(), to_wei(1000)).await;
    market_order_token
        .mint(solver.address(), to_wei(1000))
        .await;
    partner_fee_order_token
        .mint(solver.address(), to_wei(1000))
        .await;
    tx!(
        solver.account(),
        limit_order_token.approve(
            onchain.contracts().uniswap_v2_router.address(),
            to_wei(1000)
        )
    );
    tx!(
        solver.account(),
        market_order_token.approve(
            onchain.contracts().uniswap_v2_router.address(),
            to_wei(1000)
        )
    );
    tx!(
        solver.account(),
        partner_fee_order_token.approve(
            onchain.contracts().uniswap_v2_router.address(),
            to_wei(1000)
        )
    );
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

    let services = Services::new(onchain.contracts()).await;
    let solver_endpoint =
        colocation::start_baseline_solver(onchain.contracts().weth.address()).await;
    colocation::start_driver(
        onchain.contracts(),
        vec![SolverEngine {
            name: "test_solver".into(),
            account: solver.clone(),
            endpoint: solver_endpoint,
        }],
    );
    services
        .start_api(vec![
            "--price-estimation-drivers=test_quoter|http://localhost:11088/test_solver".to_string(),
        ])
        .await;

    tracing::info!("Acquiring quotes.");
    let quote_valid_to = model::time::now_in_epoch_seconds() + 300;
    let sell_amount = to_wei(10);
    let limit_quote_before = get_quote(
        &services,
        onchain.contracts().weth.address(),
        limit_order_token.address(),
        sell_amount,
        quote_valid_to,
    )
    .await
    .unwrap();
    let market_quote_before = get_quote(
        &services,
        onchain.contracts().weth.address(),
        market_order_token.address(),
        sell_amount,
        quote_valid_to,
    )
    .await
    .unwrap();
    let partner_fee_quote_before = get_quote(
        &services,
        onchain.contracts().weth.address(),
        partner_fee_order_token.address(),
        sell_amount,
        quote_valid_to,
    )
    .await
    .unwrap();

    let market_price_improvement_order = OrderCreation {
        sell_token: onchain.contracts().weth.address(),
        sell_amount,
        buy_token: market_order_token.address(),
        // to make sure the order is in-market
        buy_amount: market_quote_before.quote.buy_amount * 2 / 3,
        valid_to: quote_valid_to,
        kind: OrderKind::Sell,
        quote_id: market_quote_before.id,
        ..Default::default()
    }
    .sign(
        EcdsaSigningScheme::Eip712,
        &onchain.contracts().domain_separator,
        SecretKeyRef::from(&SecretKey::from_slice(trader.private_key()).unwrap()),
    );
    let limit_surplus_order = OrderCreation {
        sell_token: onchain.contracts().weth.address(),
        sell_amount,
        buy_token: limit_order_token.address(),
        // to make sure the order is out-of-market
        buy_amount: limit_quote_before.quote.buy_amount * 3 / 2,
        valid_to: quote_valid_to,
        kind: OrderKind::Sell,
        quote_id: limit_quote_before.id,
        ..Default::default()
    }
    .sign(
        EcdsaSigningScheme::Eip712,
        &onchain.contracts().domain_separator,
        SecretKeyRef::from(&SecretKey::from_slice(trader.private_key()).unwrap()),
    );
    let partner_fee_order = OrderCreation {
        sell_token: onchain.contracts().weth.address(),
        sell_amount,
        buy_token: partner_fee_order_token.address(),
        buy_amount: partner_fee_quote_before.quote.buy_amount * 2 / 3,
        valid_to: quote_valid_to,
        kind: OrderKind::Sell,
        app_data: partner_fee_app_data,
        quote_id: partner_fee_quote_before.id,
        ..Default::default()
    }
    .sign(
        EcdsaSigningScheme::Eip712,
        &onchain.contracts().domain_separator,
        SecretKeyRef::from(&SecretKey::from_slice(trader.private_key()).unwrap()),
    );

    tracing::info!("Rebalancing market and partner order tokens AMM pool.");
    onchain
        .mint_token_to_weth_uni_v2_pool(&market_order_token, to_wei(1000))
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
            sell_amount,
            model::time::now_in_epoch_seconds() + 300,
        )
        .await
        .unwrap();
        let new_partner_fee_order_quote = get_quote(
            &services,
            onchain.contracts().weth.address(),
            partner_fee_order_token.address(),
            sell_amount,
            model::time::now_in_epoch_seconds() + 300,
        )
        .await
        .unwrap();
        new_market_order_quote.quote.buy_amount != market_quote_before.quote.buy_amount
            && new_partner_fee_order_quote.quote.buy_amount
                != partner_fee_quote_before.quote.buy_amount
    })
    .await
    .unwrap();

    let market_quote_after = get_quote(
        &services,
        onchain.contracts().weth.address(),
        market_order_token.address(),
        sell_amount,
        model::time::now_in_epoch_seconds() + 300,
    )
    .await
    .unwrap()
    .quote;
    let partner_fee_quote_after = get_quote(
        &services,
        onchain.contracts().weth.address(),
        partner_fee_order_token.address(),
        sell_amount,
        model::time::now_in_epoch_seconds() + 300,
    )
    .await
    .unwrap()
    .quote;

    let market_price_improvement_uid = services
        .create_order(&market_price_improvement_order)
        .await
        .unwrap();
    let limit_surplus_order_uid = services.create_order(&limit_surplus_order).await.unwrap();
    let partner_fee_order_uid = services.create_order(&partner_fee_order).await.unwrap();

    let mut config = vec![
        "--drivers=test_solver|http://localhost:11088/test_solver".to_string(),
        "--price-estimation-drivers=test_quoter|http://localhost:11088/test_solver".to_string(),
    ];
    config.extend(autopilot_config);
    services.start_autopilot(None, config);

    tracing::info!("Rebalancing limit order AMM pool.");
    onchain
        .mint_token_to_weth_uni_v2_pool(&limit_order_token, to_wei(1000))
        .await;

    tracing::info!("Waiting for liquidity state to update");
    wait_for_condition(TIMEOUT, || async {
        // Mint blocks until we evict the cached liquidity and fetch the new state.
        onchain.mint_block().await;
        let new_limit_order_quote = get_quote(
            &services,
            onchain.contracts().weth.address(),
            limit_order_token.address(),
            sell_amount,
            model::time::now_in_epoch_seconds() + 300,
        )
        .await
        .unwrap();
        new_limit_order_quote.quote.buy_amount != limit_quote_before.quote.buy_amount
    })
    .await
    .unwrap();
    let limit_quote_after = get_quote(
        &services,
        onchain.contracts().weth.address(),
        limit_order_token.address(),
        sell_amount,
        model::time::now_in_epoch_seconds() + 300,
    )
    .await
    .unwrap()
    .quote;

    tracing::info!("Waiting for orders metadata update.");
    let metadata_updated = || async {
        onchain.mint_block().await;
        let market_order_updated = services
            .get_order(&market_price_improvement_uid)
            .await
            .is_ok_and(|order| !order.metadata.executed_surplus_fee.is_zero());
        let limit_order_updated = services
            .get_order(&limit_surplus_order_uid)
            .await
            .is_ok_and(|order| !order.metadata.executed_surplus_fee.is_zero());
        let partner_fee_order_updated = services
            .get_order(&partner_fee_order_uid)
            .await
            .is_ok_and(|order| !order.metadata.executed_surplus_fee.is_zero());
        market_order_updated && limit_order_updated && partner_fee_order_updated
    };
    wait_for_condition(TIMEOUT, metadata_updated).await.unwrap();

    let market_price_improvement_order = services
        .get_order(&market_price_improvement_uid)
        .await
        .unwrap();
    let market_executed_surplus_fee_in_buy_token =
        market_price_improvement_order.metadata.executed_surplus_fee
            * market_quote_after.buy_amount
            / market_quote_after.sell_amount;
    let market_quote_diff = market_quote_after
        .buy_amount
        .saturating_sub(market_quote_before.quote.buy_amount);
    assert!(market_executed_surplus_fee_in_buy_token >= market_quote_diff * 3 / 10);

    let partner_fee_order = services.get_order(&partner_fee_order_uid).await.unwrap();
    let partner_fee_executed_surplus_fee_in_buy_token =
        partner_fee_order.metadata.executed_surplus_fee * partner_fee_quote_after.buy_amount
            / partner_fee_quote_after.sell_amount;
    assert!(
        partner_fee_executed_surplus_fee_in_buy_token
            >= partner_fee_quote_after.buy_amount * 2 / 100
    );

    let limit_surplus_order = services.get_order(&limit_surplus_order_uid).await.unwrap();
    let limit_executed_surplus_fee_in_buy_token = limit_surplus_order.metadata.executed_surplus_fee
        * limit_quote_after.buy_amount
        / limit_quote_after.sell_amount;
    let limit_quote_diff = limit_quote_after
        .buy_amount
        .saturating_sub(limit_surplus_order.data.buy_amount);
    assert!(limit_executed_surplus_fee_in_buy_token >= limit_quote_diff * 3 / 10);

    let balance_after = market_order_token
        .balance_of(onchain.contracts().gp_settlement.address())
        .call()
        .await
        .unwrap();
    assert_approximately_eq!(market_executed_surplus_fee_in_buy_token, balance_after);

    let balance_after = limit_order_token
        .balance_of(onchain.contracts().gp_settlement.address())
        .call()
        .await
        .unwrap();
    assert_approximately_eq!(limit_executed_surplus_fee_in_buy_token, balance_after);

    let balance_after = partner_fee_order_token
        .balance_of(onchain.contracts().gp_settlement.address())
        .call()
        .await
        .unwrap();
    assert_approximately_eq!(partner_fee_executed_surplus_fee_in_buy_token, balance_after);
}

async fn get_quote(
    services: &Services<'_>,
    sell_token: Address,
    buy_token: Address,
    sell_amount: U256,
    valid_to: u32,
) -> Result<OrderQuoteResponse, (StatusCode, String)> {
    let quote_request = OrderQuoteRequest {
        sell_token,
        buy_token,
        side: OrderQuoteSide::Sell {
            sell_amount: SellAmount::BeforeFee {
                value: NonZeroU256::try_from(sell_amount.as_u128()).unwrap(),
            },
        },
        validity: Validity::To(valid_to),
        ..Default::default()
    };
    services.submit_quote(&quote_request).await
}

async fn volume_fee_buy_order_test(web3: Web3) {
    let fee_policy = FeePolicyKind::Volume { factor: 0.1 };
    let protocol_fee = ProtocolFee {
        policy: fee_policy,
        // The order is in-market, but specifying `Any` order class to make sure it is properly
        // applied
        policy_order_class: FeePolicyOrderClass::Any,
    };
    // Without protocol fee:
    // Expected execution is 5040413426236634210 GNO for 5000000000000000000 DAI,
    // with executed_surplus_fee = 167058994203399 GNO
    //
    // With protocol fee:
    // Expected executed_surplus_fee is 167058994203399 + 0.1*5040413426236634210 =
    // 504208401617866820
    //
    // Settlement contract balance after execution = executed_surplus_fee GNO
    execute_test(
        web3.clone(),
        vec![ProtocolFeesConfig(vec![protocol_fee])],
        OrderKind::Buy,
        None,
        504208401617866820u128.into(),
        504208401617866820u128.into(),
    )
    .await;
}

async fn execute_test(
    web3: Web3,
    autopilot_config: Vec<impl ToString>,
    order_kind: OrderKind,
    app_data: Option<OrderCreationAppData>,
    expected_surplus_fee: U256,
    expected_settlement_contract_balance: U256,
) {
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
    let solver_endpoint =
        colocation::start_baseline_solver(onchain.contracts().weth.address()).await;
    colocation::start_driver(
        onchain.contracts(),
        vec![SolverEngine {
            name: "test_solver".into(),
            account: solver,
            endpoint: solver_endpoint,
        }],
    );
    let mut config = vec![
        "--drivers=test_solver|http://localhost:11088/test_solver".to_string(),
        "--price-estimation-drivers=test_quoter|http://localhost:11088/test_solver".to_string(),
    ];
    config.extend(autopilot_config.iter().map(ToString::to_string));
    services.start_autopilot(None, config);
    services
        .start_api(vec![
            "--price-estimation-drivers=test_quoter|http://localhost:11088/test_solver".to_string(),
        ])
        .await;

    let order = OrderCreation {
        sell_token: token_gno.address(),
        sell_amount: to_wei(10),
        buy_token: token_dai.address(),
        buy_amount: to_wei(5),
        valid_to: model::time::now_in_epoch_seconds() + 300,
        app_data: app_data.unwrap_or_default(),
        kind: order_kind,
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
    assert_approximately_eq!(order.metadata.executed_surplus_fee, expected_surplus_fee);

    // Check settlement contract balance
    let balance_after = match order_kind {
        OrderKind::Buy => token_gno
            .balance_of(onchain.contracts().gp_settlement.address())
            .call()
            .await
            .unwrap(),
        OrderKind::Sell => token_dai
            .balance_of(onchain.contracts().gp_settlement.address())
            .call()
            .await
            .unwrap(),
    };
    assert_approximately_eq!(balance_after, expected_settlement_contract_balance);
}

struct ProtocolFeesConfig(Vec<ProtocolFee>);

struct ProtocolFee {
    policy: FeePolicyKind,
    policy_order_class: FeePolicyOrderClass,
}

enum FeePolicyOrderClass {
    Market,
    Limit,
    Any,
}

impl std::fmt::Display for FeePolicyOrderClass {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FeePolicyOrderClass::Market => write!(f, "market"),
            FeePolicyOrderClass::Limit => write!(f, "limit"),
            FeePolicyOrderClass::Any => write!(f, "any"),
        }
    }
}

#[derive(Clone)]
enum FeePolicyKind {
    /// How much of the order's surplus should be taken as a protocol fee.
    Surplus { factor: f64, max_volume_factor: f64 },
    /// How much of the order's volume should be taken as a protocol fee.
    Volume { factor: f64 },
    /// How much of the order's price improvement should be taken as a protocol
    /// fee where price improvement is a difference between the executed price
    /// and the best quote.
    PriceImprovement { factor: f64, max_volume_factor: f64 },
}

impl std::fmt::Display for ProtocolFee {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let order_class_str = &self.policy_order_class.to_string();
        match &self.policy {
            FeePolicyKind::Surplus {
                factor,
                max_volume_factor,
            } => write!(
                f,
                "surplus:{}:{}:{}",
                factor, max_volume_factor, order_class_str
            ),
            FeePolicyKind::Volume { factor } => {
                write!(f, "volume:{}:{}", factor, order_class_str)
            }
            FeePolicyKind::PriceImprovement {
                factor,
                max_volume_factor,
            } => write!(
                f,
                "priceImprovement:{}:{}:{}",
                factor, max_volume_factor, order_class_str
            ),
        }
    }
}

impl std::fmt::Display for ProtocolFeesConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let fees_str = self
            .0
            .iter()
            .map(|fee| fee.to_string())
            .collect::<Vec<_>>()
            .join(",");
        write!(f, "--fee-policies={}", fees_str)
    }
}
