use {
    e2e::{
        setup::{colocation::SolverEngine, *},
        tx,
    },
    ethcontract::prelude::U256,
    model::{
        order::{OrderClass, OrderCreation, OrderKind},
        signature::EcdsaSigningScheme,
    },
    secp256k1::SecretKey,
    shared::ethrpc::Web3,
    solvers_dto::solution::{Asset, Solution},
    std::collections::HashMap,
    web3::signing::SecretKeyRef,
};

#[tokio::test]
#[ignore]
async fn local_node_single_limit_order() {
    run_test(single_limit_order_test).await;
}

async fn single_limit_order_test(web3: Web3) {
    let mut onchain = OnchainComponents::deploy(web3.clone()).await;

    let [solver] = onchain.make_solvers(to_wei(100)).await;
    let [trader_a] = onchain.make_accounts(to_wei(100)).await;
    let [token_a, token_b] = onchain
        .deploy_tokens_with_weth_uni_v2_pools(to_wei(1_000), to_wei(1_000))
        .await;

    // Fund trader account
    token_a.mint(trader_a.address(), to_wei(100)).await;
    token_b.mint(trader_a.address(), to_wei(100)).await;

    // Fund solver account
    token_a.mint(solver.address(), to_wei(1000)).await;
    token_b.mint(solver.address(), to_wei(1000)).await;

    // Create and fund Uniswap pool
    tx!(
        solver.account(),
        onchain
            .contracts()
            .uniswap_v2_factory
            .create_pair(token_a.address(), token_b.address())
    );
    tx!(
        solver.account(),
        token_a.approve(
            onchain.contracts().uniswap_v2_router.address(),
            to_wei(1000)
        )
    );
    tx!(
        solver.account(),
        token_b.approve(
            onchain.contracts().uniswap_v2_router.address(),
            to_wei(1000)
        )
    );
    tx!(
        solver.account(),
        onchain.contracts().uniswap_v2_router.add_liquidity(
            token_a.address(),
            token_b.address(),
            to_wei(100),
            to_wei(100),
            0_u64.into(),
            0_u64.into(),
            solver.address(),
            U256::max_value(),
        )
    );

    // Approve GPv2 for trading
    tx!(
        trader_a.account(),
        token_a.approve(onchain.contracts().allowance, to_wei(100))
    );
    tx!(
        trader_a.account(),
        token_b.approve(onchain.contracts().allowance, to_wei(100))
    );
    tx!(
        solver.account(),
        token_a.approve(onchain.contracts().allowance, to_wei(100))
    );
    tx!(
        solver.account(),
        token_b.approve(onchain.contracts().allowance, to_wei(100))
    );

    // Place Orders
    let services = Services::new(onchain.contracts()).await;

    let solution = Solution {
        id: 0,
        prices: HashMap::from([
            (token_a.address(), to_wei(1)),
            (token_b.address(), to_wei(1)),
        ]),
        trades: vec![
            solvers_dto::solution::Trade::Jit(solvers_dto::solution::JitTrade {
                order: JitOrder {
                    owner: trader_a.address(),
                    sell: Asset {
                        amount: to_wei(10),
                        token: token_b.address(),
                    },
                    buy: Asset {
                        amount: to_wei(1),
                        token: token_a.address(),
                    },
                    kind: OrderKind::Sell,
                    partially_fillable: false,
                    valid_to: model::time::now_in_epoch_seconds() + 300,
                    app_data: Default::default(),
                    receiver: solver.address(),
                }
                .sign(
                    EcdsaSigningScheme::Eip712,
                    &onchain.contracts().domain_separator,
                    SecretKeyRef::from(&SecretKey::from_slice(solver.private_key()).unwrap()),
                ),
                executed_amount: to_wei(10),
            }),
            solvers_dto::solution::Trade::Fulfillment(solvers_dto::solution::Fulfillment {
                executed_amount: to_wei(10),
                fee: Some(0.into()),
                // Dummy as it will be overwritten in the solver
                order: [0; 56],
            }),
        ],
        pre_interactions: vec![],
        interactions: vec![],
        post_interactions: vec![],
        gas: None,
    };

    // Start system
    colocation::start_driver(
        onchain.contracts(),
        vec![
            SolverEngine {
                name: "test_solver".into(),
                account: solver.clone(),
                endpoint: colocation::start_baseline_solver(onchain.contracts().weth.address())
                    .await,
            },
            SolverEngine {
                name: "mock_solver".into(),
                account: solver.clone(),
                endpoint: colocation::start_mock_solver(solution).await,
            },
        ],
        colocation::LiquidityProvider::UniswapV2,
    );

    // We start the quoter as the baseline solver, and the mock solver as the one
    // returning the solution
    services
        .start_autopilot(
            None,
            vec![
                "--drivers=mock_solver|http://localhost:11088/mock_solver".to_string(),
                "--price-estimation-drivers=test_solver|http://localhost:11088/test_solver"
                    .to_string(),
            ],
        )
        .await;
    services
        .start_api(vec![
            "--price-estimation-drivers=test_solver|http://localhost:11088/test_solver".to_string(),
        ])
        .await;

    let order = OrderCreation {
        sell_token: token_a.address(),
        sell_amount: to_wei(10),
        buy_token: token_b.address(),
        buy_amount: to_wei(5),
        valid_to: model::time::now_in_epoch_seconds() + 300,
        kind: OrderKind::Sell,
        ..Default::default()
    }
    .sign(
        EcdsaSigningScheme::Eip712,
        &onchain.contracts().domain_separator,
        SecretKeyRef::from(&SecretKey::from_slice(trader_a.private_key()).unwrap()),
    );
    let order_id = services.create_order(&order).await.unwrap();
    let limit_order = services.get_order(&order_id).await.unwrap();
    assert_eq!(limit_order.metadata.class, OrderClass::Limit);

    // Drive solution
    tracing::info!("Waiting for trade.");
    let trader_balance_before = token_b.balance_of(trader_a.address()).call().await.unwrap();
    let solver_balance_before = token_b.balance_of(solver.address()).call().await.unwrap();
    wait_for_condition(TIMEOUT, || async { services.solvable_orders().await == 1 })
        .await
        .unwrap();

    wait_for_condition(TIMEOUT, || async { services.solvable_orders().await == 0 })
        .await
        .unwrap();

    let trader_balance_after = token_b.balance_of(trader_a.address()).call().await.unwrap();
    let solver_balance_after = token_b.balance_of(solver.address()).call().await.unwrap();

    assert!(
        trader_balance_after
            .checked_sub(trader_balance_before)
            .unwrap()
            >= to_wei(5)
    );
    // Since the fee is 0 in the custom solution, the balance difference has to be
    // exactly 10 wei
    assert_eq!(
        solver_balance_before
            .checked_sub(solver_balance_after)
            .unwrap(),
        to_wei(10)
    );
}
