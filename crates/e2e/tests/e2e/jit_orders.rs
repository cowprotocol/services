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

    let services = Services::new(&onchain).await;

    let mock_solver = Mock::default();

    // Start system
    colocation::start_driver(
        onchain.contracts(),
        vec![
            colocation::start_baseline_solver(
                "test_solver".into(),
                solver.clone(),
                onchain.contracts().weth.address(),
                vec![],
                1,
                true,
            )
            .await,
            SolverEngine {
                name: "mock_solver".into(),
                account: solver.clone(),
                endpoint: mock_solver.url.clone(),
                base_tokens: vec![token.address()],
                merge_solutions: true,
            },
        ],
        colocation::LiquidityProvider::UniswapV2,
        false,
    );

    // We start the quoter as the baseline solver, and the mock solver as the one
    // returning the solution
    services
        .start_autopilot(
            None,
            vec![
                format!(
                    "--drivers=mock_solver|http://localhost:11088/mock_solver|{}",
                    hex::encode(solver.address())
                ),
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

    let trader_balance_before = token.balance_of(trader.address()).call().await.unwrap();
    let solver_balance_before = token.balance_of(solver.address()).call().await.unwrap();
    let order_id = services.create_order(&order).await.unwrap();
    let limit_order = services.get_order(&order_id).await.unwrap();
    onchain.mint_block().await;
    assert_eq!(limit_order.metadata.class, OrderClass::Limit);

    let (jit_order, jit_order_uid) = JitOrder {
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
    );

    mock_solver.configure_solution(Some(Solution {
        id: 0,
        prices: HashMap::from([
            (token.address(), to_wei(1)),
            (onchain.contracts().weth.address(), to_wei(1)),
        ]),
        trades: vec![
            solvers_dto::solution::Trade::Jit(solvers_dto::solution::JitTrade {
                order: jit_order,
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
        flashloans: vec![],
    }));

    // Drive solution
    tracing::info!("Waiting for trade.");
    onchain.mint_block().await;
    wait_for_condition(TIMEOUT, || async {
        let trader_balance_after = token.balance_of(trader.address()).call().await.unwrap();
        let solver_balance_after = token.balance_of(solver.address()).call().await.unwrap();

        let trader_balance_increased =
            trader_balance_after.saturating_sub(trader_balance_before) >= to_wei(5);
        // Since the fee is 0 in the custom solution, the balance difference has to be
        // exactly 10 wei
        let solver_balance_decreased =
            solver_balance_before.saturating_sub(solver_balance_after) == to_wei(10);
        trader_balance_increased && solver_balance_decreased
    })
    .await
    .unwrap();

    tracing::info!("Waiting for trade to be indexed.");
    wait_for_condition(TIMEOUT, || async {
        // jit order can be found on /api/v1/orders
        services.get_order(&jit_order_uid).await.ok()?;

        // jit order can be found on /api/v1/trades
        let tx_hash = services
            .get_trades(&jit_order_uid)
            .await
            .ok()?
            .pop()?
            .tx_hash?;

        // jit order can be found on /api/v1/transactions/{tx_hash}/orders
        let orders_by_tx = services.get_orders_for_tx(&tx_hash).await.ok()?;

        // jit order can be found on /api/v1/account/{owner}/orders
        let orders_by_owner = services
            .get_orders_for_owner(&jit_order_uid.parts().1, 0, 10)
            .await
            .ok()?;
        let jit_order_by_owner = orders_by_owner
            .iter()
            .any(|o| o.metadata.uid == jit_order_uid);
        let jit_order_by_tx = orders_by_tx.iter().any(|o| o.metadata.uid == jit_order_uid);
        Some(jit_order_by_owner && jit_order_by_tx)
    })
    .await
    .unwrap();

    // make sure the offset works
    let orders_by_owner = services
        .get_orders_for_owner(&jit_order_uid.parts().1, 1, 1)
        .await
        .unwrap();
    assert!(orders_by_owner.is_empty());
}
