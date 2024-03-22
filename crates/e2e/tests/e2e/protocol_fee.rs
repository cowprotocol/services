use {
    e2e::{
        setup::{colocation::SolverEngine, *},
        tx,
    },
    ethcontract::prelude::U256,
    model::{
        order::{OrderCreation, OrderCreationAppData, OrderKind},
        signature::EcdsaSigningScheme,
    },
    secp256k1::SecretKey,
    serde_json::json,
    shared::ethrpc::Web3,
    web3::signing::SecretKeyRef,
};

#[tokio::test]
#[ignore]
async fn local_node_surplus_fee_sell_order() {
    run_test(surplus_fee_sell_order_test).await;
}

#[tokio::test]
#[ignore]
async fn local_node_surplus_fee_sell_order_capped() {
    run_test(surplus_fee_sell_order_capped_test).await;
}

#[tokio::test]
#[ignore]
async fn local_node_volume_fee_sell_order() {
    run_test(volume_fee_sell_order_test).await;
}

#[tokio::test]
#[ignore]
async fn local_node_partner_fee_sell_order() {
    run_test(partner_fee_sell_order_test).await;
}

#[tokio::test]
#[ignore]
async fn local_node_surplus_fee_buy_order() {
    run_test(surplus_fee_buy_order_test).await;
}

#[tokio::test]
#[ignore]
async fn local_node_surplus_fee_buy_order_capped() {
    run_test(surplus_fee_buy_order_capped_test).await;
}

#[tokio::test]
#[ignore]
async fn local_node_volume_fee_buy_order() {
    run_test(volume_fee_buy_order_test).await;
}

#[tokio::test]
#[ignore]
async fn local_node_price_improvement_fee_sell_order() {
    run_test(price_improvement_fee_sell_order_test).await;
}

async fn surplus_fee_sell_order_test(web3: Web3) {
    let fee_policy = FeePolicyKind::Surplus {
        factor: 0.3,
        max_volume_factor: 0.9,
    };
    let protocol_fee = ProtocolFee {
        policy: fee_policy,
        policy_order_class: FeePolicyOrderClass::Market,
    };
    // Without protocol fee:
    // Expected execution is 10000000000000000000 GNO for
    // 9871415430342266811 DAI, with executed_surplus_fee = 167058994203399 GNO
    //
    // With protocol fee:
    // surplus [DAI] = 9871415430342266811 DAI - 5000000000000000000 DAI =
    // 4871415430342266811 DAI
    //
    // protocol fee = 0.3*surplus = 1461424629102680043 DAI =
    // 1461424629102680043 DAI / 9871415430342266811 *
    // (10000000000000000000 - 167058994203399) = 1480436341679873337 GNO
    //
    // final execution is 10000000000000000000 GNO for 8409990801239586768 DAI, with
    // executed_surplus_fee = 1480603400674076736 GNO
    //
    // Settlement contract balance after execution = 1480603400674076736 GNO =
    // 1480603400674076736 GNO * 8409990801239586768 / (10000000000000000000 -
    // 1480603400674076736) = 1461589542731026166 DAI
    execute_test(
        web3.clone(),
        vec![protocol_fee],
        OrderKind::Sell,
        None,
        1480603400674076736u128.into(),
        1461589542731026166u128.into(),
    )
    .await;
}

async fn surplus_fee_sell_order_capped_test(web3: Web3) {
    let fee_policy = FeePolicyKind::Surplus {
        factor: 0.9,
        max_volume_factor: 0.1,
    };
    let protocol_fee = ProtocolFee {
        policy: fee_policy,
        policy_order_class: FeePolicyOrderClass::Market,
    };
    // Without protocol fee:
    // Expected execution is 10000000000000000000 GNO for
    // 9871415430342266811 DAI, with executed_surplus_fee = 167058994203399 GNO
    //
    // With protocol fee:
    // Expected executed_surplus_fee is 167058994203399 +
    // 0.1*(10000000000000000000 - 167058994203399) = 1000150353094783059
    //
    // Final execution is 10000000000000000000 GNO for 8884273887308040129 DAI, with
    // executed_surplus_fee = 1000150353094783059 GNO
    //
    // Settlement contract balance after execution = 1000150353094783059 GNO =
    // 1000150353094783059 GNO * 8884273887308040129 / (10000000000000000000 -
    // 1000150353094783059) = 987306456662572858 DAI
    execute_test(
        web3.clone(),
        vec![protocol_fee],
        OrderKind::Sell,
        None,
        1000150353094783059u128.into(),
        987306456662572858u128.into(),
    )
    .await;
}

async fn volume_fee_sell_order_test(web3: Web3) {
    let fee_policy = FeePolicyKind::Volume { factor: 0.1 };
    let protocol_fee = ProtocolFee {
        policy: fee_policy,
        policy_order_class: FeePolicyOrderClass::Market,
    };
    // Without protocol fee:
    // Expected execution is 10000000000000000000 GNO for
    // 9871415430342266811 DAI, with executed_surplus_fee = 167058994203399 GNO
    //
    // With protocol fee:
    // Expected executed_surplus_fee is 167058994203399 +
    // 0.1*(10000000000000000000 - 167058994203399) = 1000150353094783059
    //
    // Final execution is 10000000000000000000 GNO for 8884273887308040129 DAI, with
    // executed_surplus_fee = 1000150353094783059 GNO
    //
    // Settlement contract balance after execution = 1000150353094783059 GNO =
    // 1000150353094783059 GNO * 8884273887308040129 / (10000000000000000000 -
    // 1000150353094783059) = 987306456662572858 DAI
    execute_test(
        web3.clone(),
        vec![protocol_fee],
        OrderKind::Sell,
        None,
        1000150353094783059u128.into(),
        987306456662572858u128.into(),
    )
    .await;
}

async fn partner_fee_sell_order_test(web3: Web3) {
    // Fee policy to be overwritten by the partner fee + capped to 0.01
    let fee_policy = FeePolicyKind::PriceImprovement {
        factor: 0.5,
        max_volume_factor: 0.9,
    };
    let protocol_fee = ProtocolFee {
        policy: fee_policy,
        policy_order_class: FeePolicyOrderClass::Market,
    };
    // Without protocol fee:
    // Expected execution is 10000000000000000000 GNO for
    // 9871415430342266811 DAI, with executed_surplus_fee = 167058994203399 GNO
    //
    // With protocol fee:
    // Expected executed_surplus_fee is 167058994203399 +
    // 0.01*(10000000000000000000 - 167058994203399) = 100165388404261365
    //
    // Final execution is 10000000000000000000 GNO for 9772701276038844388 DAI, with
    // executed_surplus_fee = 100165388404261365 GNO
    //
    // Settlement contract balance after execution = 100165388404261365 GNO =
    // 100165388404261365 GNO * 9772701276038844388 / (10000000000000000000 -
    // 100165388404261365) = 98879067931768848 DAI
    execute_test(
        web3.clone(),
        vec![protocol_fee],
        OrderKind::Sell,
        Some(OrderCreationAppData::Full {
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
        }),
        100165388404261365u128.into(),
        98879067931768848u128.into(),
    )
    .await;
}

async fn surplus_fee_buy_order_test(web3: Web3) {
    let fee_policy = FeePolicyKind::Surplus {
        factor: 0.3,
        max_volume_factor: 0.9,
    };
    let protocol_fee = ProtocolFee {
        policy: fee_policy,
        policy_order_class: FeePolicyOrderClass::Market,
    };
    // Without protocol fee:
    // Expected execution is 5040413426236634210 GNO for 5000000000000000000 DAI,
    // with executed_surplus_fee = 167058994203399 GNO
    //
    // With protocol fee:
    // surplus in sell token = 10000000000000000000 - 5040413426236634210 =
    // 4959586573763365790
    //
    // protocol fee in sell token = 0.3*4959586573763365790 = 1487875972129009737
    //
    // expected executed_surplus_fee is 167058994203399 + 1487875972129009737 =
    // 1488043031123213136
    //
    // Settlement contract balance after execution = executed_surplus_fee GNO
    execute_test(
        web3.clone(),
        vec![protocol_fee],
        OrderKind::Buy,
        None,
        1488043031123213136u128.into(),
        1488043031123213136u128.into(),
    )
    .await;
}

async fn surplus_fee_buy_order_capped_test(web3: Web3) {
    let fee_policy = FeePolicyKind::Surplus {
        factor: 0.9,
        max_volume_factor: 0.1,
    };
    let protocol_fee = ProtocolFee {
        policy: fee_policy,
        policy_order_class: FeePolicyOrderClass::Market,
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
        vec![protocol_fee],
        OrderKind::Buy,
        None,
        504208401617866820u128.into(),
        504208401617866820u128.into(),
    )
    .await;
}

async fn volume_fee_buy_order_test(web3: Web3) {
    let fee_policy = FeePolicyKind::Volume { factor: 0.1 };
    let protocol_fee = ProtocolFee {
        policy: fee_policy,
        policy_order_class: FeePolicyOrderClass::Market,
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
        vec![protocol_fee],
        OrderKind::Buy,
        None,
        504208401617866820u128.into(),
        504208401617866820u128.into(),
    )
    .await;
}

async fn price_improvement_fee_sell_order_test(web3: Web3) {
    let fee_policy = FeePolicyKind::PriceImprovement {
        factor: 0.3,
        max_volume_factor: 0.9,
    };
    let protocol_fee = ProtocolFee {
        policy: fee_policy,
        policy_order_class: FeePolicyOrderClass::Market,
    };
    // Without protocol fee:
    // Expected execution is 10000000000000000000 GNO for
    // 9871415430342266811 DAI, with executed_surplus_fee = 167058994203399 GNO
    //
    // Quote: 10000000000000000000 GNO for 9871580343970612988 DAI with
    // 294580438010728 GNO fee. Equivalent to: (10000000000000000000 +
    // 294580438010728) GNO for 9871580343970612988 DAI, then scaled to sell amount
    // gives 10000000000000000000 GNO for 9871289555090525964 DAI
    //
    // Price improvement over quote: 9871415430342266811 - 9871289555090525964 =
    // 125875251741847 DAI. Protocol fee = 0.3 * 125875251741847 DAI =
    // 37762575522554 DAI
    //
    // Protocol fee in sell token: 37762575522554 DAI / 9871415430342266811 *
    // (10000000000000000000 - 167058994203399) = 38253829890184 GNO
    //
    // Final execution is 10000000000000000000 GNO for (9871415430342266811 -
    // 37762575522554) = 9871377667766744257 DAI, with 205312824093583 GNO fee
    //
    // Settlement contract balance after execution = 205312824093583 GNO =
    // 205312824093583 GNO * 9871377667766744257 / (10000000000000000000 -
    // 205312824093583) = 202676203868731 DAI
    execute_test(
        web3.clone(),
        vec![protocol_fee],
        OrderKind::Sell,
        None,
        205312824093583u128.into(),
        202676203868731u128.into(),
    )
    .await;
}

// because of rounding errors, it's good enough to check that the expected value
// is within a very narrow range of the executed value
fn is_approximately_equal(executed_value: U256, expected_value: U256) -> bool {
    let lower = expected_value * U256::from(99999999999u128) / U256::from(100000000000u128); // in percents = 99.999999999%
    let upper = expected_value * U256::from(100000000001u128) / U256::from(100000000000u128); // in percents = 100.000000001%
    executed_value >= lower && executed_value <= upper
}

async fn execute_test(
    web3: Web3,
    protocol_fees: Vec<ProtocolFee>,
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
    let autopilot_args = vec![
        "--drivers=test_solver|http://localhost:11088/test_solver".to_string(),
        "--price-estimation-drivers=test_quoter|http://localhost:11088/test_solver".to_string(),
        ProtocolFeesConfig(protocol_fees).to_string(),
    ];
    services.start_autopilot(None, autopilot_args);
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
        is_approximately_equal(order.metadata.executed_surplus_fee, expected_surplus_fee)
    };
    wait_for_condition(TIMEOUT, metadata_updated).await.unwrap();

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
    assert!(is_approximately_equal(
        balance_after,
        expected_settlement_contract_balance
    ));
}

struct ProtocolFeesConfig(Vec<ProtocolFee>);

struct ProtocolFee {
    policy: FeePolicyKind,
    policy_order_class: FeePolicyOrderClass,
}

enum FeePolicyOrderClass {
    Market,
    #[allow(dead_code)]
    Limit,
}

impl std::fmt::Display for FeePolicyOrderClass {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FeePolicyOrderClass::Market => write!(f, "market"),
            FeePolicyOrderClass::Limit => write!(f, "limit"),
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
            .join("|");
        write!(f, "--fee-policies={}", fees_str)
    }
}
