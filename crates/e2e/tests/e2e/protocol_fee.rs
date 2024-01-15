use {
    e2e::{
        setup::{colocation::SolverEngine, *},
        tx,
    },
    ethcontract::prelude::U256,
    model::{
        order::{OrderCreation, OrderKind},
        signature::EcdsaSigningScheme,
    },
    secp256k1::SecretKey,
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

async fn surplus_fee_sell_order_test(web3: Web3) {
    let fee_policy = FeePolicyKind::Surplus {
        factor: 0.3,
        max_volume_factor: 1.0,
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
        fee_policy,
        OrderKind::Sell,
        1480603400674076736u128.into(),
        1461589542731026166u128.into(),
    )
    .await;
}

async fn surplus_fee_sell_order_capped_test(web3: Web3) {
    let fee_policy = FeePolicyKind::Surplus {
        factor: 1.0,
        max_volume_factor: 0.1,
    };
    // Without protocol fee:
    // Expected executed_surplus_fee is 167058994203399
    //
    // With protocol fee:
    // Expected executed_surplus_fee is 167058994203399 +
    // 0.1*10000000000000000000 = 1000167058994203400
    //
    // Final execution is 10000000000000000000 GNO for 8884257395945205588 DAI, with
    // executed_surplus_fee = 1000167058994203400 GNO
    //
    // Settlement contract balance after execution = 1000167058994203400 GNO =
    // 1000167058994203400 GNO * 8884257395945205588 / (10000000000000000000 -
    // 1000167058994203400) = 987322948025407485 DAI
    execute_test(
        web3.clone(),
        fee_policy,
        OrderKind::Sell,
        1000167058994203400u128.into(),
        987322948025407485u128.into(),
    )
    .await;
}

async fn volume_fee_sell_order_test(web3: Web3) {
    let fee_policy = FeePolicyKind::Volume { factor: 0.1 };
    // Without protocol fee:
    // Expected executed_surplus_fee is 167058994203399
    //
    // With protocol fee:
    // Expected executed_surplus_fee is 167058994203399 +
    // 0.1*10000000000000000000 = 1000167058994203400
    //
    // Settlement contract balance after execution = 1000167058994203400 GNO =
    // 1000167058994203400 GNO * 8884257395945205588 / (10000000000000000000 -
    // 1000167058994203400) = 987322948025407485 DAI
    execute_test(
        web3.clone(),
        fee_policy,
        OrderKind::Sell,
        1000167058994203400u128.into(),
        987322948025407485u128.into(),
    )
    .await;
}

async fn surplus_fee_buy_order_test(web3: Web3) {
    let fee_policy = FeePolicyKind::Surplus {
        factor: 0.3,
        max_volume_factor: 1.0,
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
        fee_policy,
        OrderKind::Buy,
        1488043031123213136u128.into(),
        1488043031123213136u128.into(),
    )
    .await;
}

async fn surplus_fee_buy_order_capped_test(web3: Web3) {
    let fee_policy = FeePolicyKind::Surplus {
        factor: 1.0,
        max_volume_factor: 0.1,
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
        fee_policy,
        OrderKind::Buy,
        504208401617866820u128.into(),
        504208401617866820u128.into(),
    )
    .await;
}

async fn volume_fee_buy_order_test(web3: Web3) {
    let fee_policy = FeePolicyKind::Volume { factor: 0.1 };
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
        fee_policy,
        OrderKind::Buy,
        504208401617866820u128.into(),
        504208401617866820u128.into(),
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
    fee_policy: FeePolicyKind,
    order_kind: OrderKind,
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
    services.start_autopilot(vec![
        "--drivers=test_solver|http://localhost:11088/test_solver".to_string(),
        "--fee-policy-skip-market-orders=false".to_string(),
        fee_policy.to_string(),
    ]);
    services
        .start_api(vec![
            "--price-estimation-drivers=test_solver|http://localhost:11088/test_solver".to_string(),
        ])
        .await;

    let order = OrderCreation {
        sell_token: token_gno.address(),
        sell_amount: to_wei(10),
        buy_token: token_dai.address(),
        buy_amount: to_wei(5),
        valid_to: model::time::now_in_epoch_seconds() + 300,
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

    onchain.mint_blocks_past_reorg_threshold().await;
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

enum FeePolicyKind {
    /// How much of the order's surplus should be taken as a protocol fee.
    Surplus { factor: f64, max_volume_factor: f64 },
    /// How much of the order's volume should be taken as a protocol fee.
    Volume { factor: f64 },
}

impl std::fmt::Display for FeePolicyKind {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            FeePolicyKind::Surplus {
                factor,
                max_volume_factor,
            } => write!(
                f,
                "--fee-policy-kind=surplus:{}:{}",
                factor, max_volume_factor
            ),
            FeePolicyKind::Volume { factor } => {
                write!(f, "--fee-policy-kind=volume:{}", factor)
            }
        }
    }
}
