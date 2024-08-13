use {
    e2e::{
        setup::{colocation::SolverEngine, mock::Mock, solution::JitOrder, *},
        tx,
        tx_value,
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
    let [trader] = onchain.make_accounts(to_wei(100)).await;
    let [token] = onchain
        .deploy_tokens_with_weth_uni_v2_pools(to_wei(300_000), to_wei(1_000))
        .await;

    token.mint(solver.address(), to_wei(100)).await;

    tx_value!(
        trader.account(),
        to_wei(20),
        onchain.contracts().weth.deposit()
    );
    tx!(
        trader.account(),
        onchain
            .contracts()
            .weth
            .approve(onchain.contracts().allowance, U256::MAX)
    );
    tx!(
        solver.account(),
        token.approve(onchain.contracts().allowance, U256::MAX)
    );

    let services = Services::new(onchain.contracts()).await;

    let mock_solver = Mock::default();

    // Start system
    colocation::start_driver(
        onchain.contracts(),
        vec![
            SolverEngine {
                name: "test_solver".into(),
                account: solver.clone(),
                endpoint: colocation::start_baseline_solver(
                    onchain.contracts().weth.address(),
                    vec![],
                )
                .await,
            },
            SolverEngine {
                name: "mock_solver".into(),
                account: solver.clone(),
                endpoint: mock_solver.url.clone(),
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

    // Place order
    let order = OrderCreation {
        sell_token: onchain.contracts().weth.address(),
        sell_amount: to_wei(10),
        buy_token: token.address(),
        buy_amount: to_wei(5),
        valid_to: model::time::now_in_epoch_seconds() + 300,
        kind: OrderKind::Sell,
        ..Default::default()
    }
    .sign(
        EcdsaSigningScheme::Eip712,
        &onchain.contracts().domain_separator,
        SecretKeyRef::from(&SecretKey::from_slice(trader.private_key()).unwrap()),
    );
    let order_id = services.create_order(&order).await.unwrap();
    let limit_order = services.get_order(&order_id).await.unwrap();
    assert_eq!(limit_order.metadata.class, OrderClass::Limit);

    mock_solver.configure_solution(Some(Solution {
        id: 0,
        prices: HashMap::from([
            (token.address(), to_wei(1)),
            (onchain.contracts().weth.address(), to_wei(1)),
        ]),
        trades: vec![
            solvers_dto::solution::Trade::Jit(solvers_dto::solution::JitTrade {
                order: JitOrder {
                    owner: trader.address(),
                    sell: Asset {
                        amount: to_wei(10),
                        token: token.address(),
                    },
                    buy: Asset {
                        amount: to_wei(1),
                        token: onchain.contracts().weth.address(),
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
                fee: Some(0.into()),
            }),
            solvers_dto::solution::Trade::Fulfillment(solvers_dto::solution::Fulfillment {
                executed_amount: order.sell_amount,
                fee: Some(0.into()),
                order: order_id.0,
            }),
        ],
        pre_interactions: vec![],
        interactions: vec![],
        post_interactions: vec![],
        gas: None,
    }));

    // Drive solution
    tracing::info!("Waiting for trade.");
    let trader_balance_before = token.balance_of(trader.address()).call().await.unwrap();
    let solver_balance_before = token.balance_of(solver.address()).call().await.unwrap();
    wait_for_condition(TIMEOUT, || async { services.solvable_orders().await == 1 })
        .await
        .unwrap();

    wait_for_condition(TIMEOUT, || async { services.solvable_orders().await == 0 })
        .await
        .unwrap();

    let trader_balance_after = token.balance_of(trader.address()).call().await.unwrap();
    let solver_balance_after = token.balance_of(solver.address()).call().await.unwrap();

    wait_for_condition(TIMEOUT, || async {
        trader_balance_after
            .checked_sub(trader_balance_before)
            .unwrap()
            >= to_wei(5)
    })
    .await
    .unwrap();

    // Since the fee is 0 in the custom solution, the balance difference has to
    // be exactly 10 wei
    wait_for_condition(TIMEOUT, || async {
        solver_balance_before
            .checked_sub(solver_balance_after)
            .unwrap()
            == to_wei(10)
    })
    .await
    .unwrap();
}
