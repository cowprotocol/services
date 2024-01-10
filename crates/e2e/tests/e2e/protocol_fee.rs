use model::order::LimitOrderClass;

use {
    e2e::{
        nodes::forked_node::ForkedNodeApi,
        setup::{colocation::SolverEngine, *},
        tx,
    },
    ethcontract::{prelude::U256, H160},
    model::{
        order::{OrderClass, OrderCreation, OrderKind},
        signature::EcdsaSigningScheme,
    },
    secp256k1::SecretKey,
    shared::ethrpc::Web3,
    web3::signing::SecretKeyRef,
};

#[tokio::test]
#[ignore]
async fn price_improvement_fee_sell_order() {
    run_test(price_improvement_fee_sell_order_test).await;
}

#[tokio::test]
#[ignore]
async fn price_improvement_fee_sell_capped_order() {
    run_test(price_improvement_fee_sell_order_capped_test).await;
}

#[tokio::test]
#[ignore]
async fn price_improvement_fee_buy_order() {
    run_test(price_improvement_fee_buy_order_test).await;
}

#[tokio::test]
#[ignore]
async fn price_improvement_fee_buy_capped_order() {
    run_test(price_improvement_fee_buy_order_capped_test).await;
}

async fn price_improvement_fee_sell_order_test(web3: Web3) {
    // without protocol fee, expected execution is 10000000000000000000 GNO for
    // 9871415430342266811 DAI, with executed_surplus_fee = 167058994203399 GNO
    // 
    // with protocol fee, 
    // surplus = 9871415430342266811 - 5000000000000000000 = 4871415430342266811
    // protocol fee in surplus token = 0.3*surplus = 1461424629102680043
    // protocol fee in sell token = 1461424629102680043 / 9871415430342266811 * (10000000000000000000 - 167058994203399) ~= 1480436341679873337
    // expected executed_surplus_fee is 167058994203399 + 1480436341679873337 = 1480603400674076736 (actually 1480603400674076783, rounding errors?)
    let (_, token_dai, trader) = prepare_test(
        web3.clone(),
        "--fee-policy-kind=priceImprovement:0.3:1.0".to_string(),
        OrderKind::Sell,
    )
    .await;

    // without protocol fee, expected execution is 10000000000000000000 GNO for
    // 9871415430342266811 DAI
    let balance_after = token_dai.balance_of(trader.address()).call().await.unwrap();
    assert_eq!(balance_after, 8409990801239586768u128.into()); // 9871415430342266811 - 0.3*(9871415430342266811 - 5000000000000000000)

    // 1480603400674076783

    // 167058994203399
    
    // onchain.mint_blocks_past_reorg_threshold().await;
    // let metadata_updated = || async {
    //     onchain.mint_block().await;
    //     let order = services.get_order(&uid).await.unwrap();
    //     !order.executed_surplus_fee.is_zero() && order.executed_surplus_fee == 1480603400674076783u128.into()
    // };
    // wait_for_condition(TIMEOUT, metadata_updated).await.unwrap();
}

async fn price_improvement_fee_sell_order_capped_test(web3: Web3) {
    // without protocol fee, expected executed_surplus_fee is 167058994203399
    // with protocol fee, expected executed_surplus_fee is 167058994203399 + 0.1*10000000000000000000 = 1000167058994203400
    //let executed_surplus_fee = 1000167058994203400u128.into();

    let (_, token_dai, trader) = prepare_test(
        web3.clone(),
        "--fee-policy-kind=priceImprovement:1.0:0.1".to_string(),
        OrderKind::Sell,
    )
    .await;
}

async fn price_improvement_fee_buy_order_test(web3: Web3) {
    // without protocol fee, expected execution is 5040413426236634210 GNO for
    // 5000000000000000000, with executed_surplus_fee = 167058994203399 GNO

    





    let (token_gno, _, trader) = prepare_test(
        web3.clone(),
        "--fee-policy-kind=priceImprovement:0.0:1.0".to_string(),
        OrderKind::Buy,
    )
    .await;

    // without protocol fee, expected execution is 5040413426236634210 GNO for
    // 5000000000000000000, with executed_surplus_fee = 167058994203399 GNO
    let balance_after = token_gno.balance_of(trader.address()).call().await.unwrap();
    // initial balance is 100 GNO
    // limit price is 10 GNO
    // expected: to_wei(100) - 5040413426236634210 - 0.3*(to_wei(10) -
    // 5040413426236634210)
    assert_eq!(balance_after, 93471710601634356053u128.into());
}

async fn price_improvement_fee_buy_order_capped_test(web3: Web3) {
    // without protocol fee, expected executed_surplus_fee is 167058994203399
    // without protocol fee, expected execution is 5040413426236634210 GNO (including fees) for
    // 5000000000000000000

    // with protocol fee, expected executed_surplus_fee is 167058994203399 + 0.1*5040413426236634210 = 504208401617866820
    
    let (token_gno, _, trader) = prepare_test(
        web3.clone(),
        "--fee-policy-kind=priceImprovement:1.0:0.1".to_string(),
        OrderKind::Buy,
    )
    .await;
}

async fn prepare_test(
    web3: Web3,
    fee_policy: String,
    order_kind: OrderKind,
) -> (MintableToken, MintableToken, TestAccount) {
    let mut onchain = OnchainComponents::deploy(web3.clone()).await;

    let [solver] = onchain.make_solvers(to_wei(1)).await;
    let [trader_a] = onchain.make_accounts(to_wei(1)).await;
    let [token_gno, token_dai] = onchain
        .deploy_tokens_with_weth_uni_v2_pools(to_wei(1_000), to_wei(1000))
        .await;

    // Fund trader accounts
    token_gno.mint(trader_a.address(), to_wei(100)).await;

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
        trader_a.account(),
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
        fee_policy,
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
        SecretKeyRef::from(&SecretKey::from_slice(trader_a.private_key()).unwrap()),
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
        let executed_surplus_fee = match order.metadata.class {
            OrderClass::Limit(LimitOrderClass {
                executed_surplus_fee,
                ..
            }) => executed_surplus_fee,
            _ => unreachable!(),
        };
        println!("executed_surplus_fee: {}", executed_surplus_fee);
        !executed_surplus_fee.is_zero()
    };
    wait_for_condition(TIMEOUT, metadata_updated).await.unwrap();

    (token_gno, token_dai, trader_a)
}
