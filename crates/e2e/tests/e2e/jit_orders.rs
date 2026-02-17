use {
    ::alloy::primitives::U256,
    autopilot::config::{
        Configuration,
        solver::{Account, Solver},
    },
    e2e::setup::{colocation::SolverEngine, mock::Mock, solution::JitOrder, *},
    ethrpc::alloy::CallBuilderExt,
    model::{
        order::{OrderClass, OrderCreation, OrderKind},
        signature::EcdsaSigningScheme,
    },
    number::units::EthUnit,
    shared::web3::Web3,
    solvers_dto::solution::{Asset, Solution},
    std::{collections::HashMap, str::FromStr},
    url::Url,
};

#[tokio::test]
#[ignore]
async fn local_node_single_limit_order() {
    run_test(single_limit_order_test).await;
}

async fn single_limit_order_test(web3: Web3) {
    let mut onchain = OnchainComponents::deploy(web3.clone()).await;

    let [solver] = onchain.make_solvers(100u64.eth()).await;
    let [trader] = onchain.make_accounts(100u64.eth()).await;
    let [token] = onchain
        .deploy_tokens_with_weth_uni_v2_pools(300_000u64.eth(), 1_000u64.eth())
        .await;

    token.mint(solver.address(), 100u64.eth()).await;

    onchain
        .contracts()
        .weth
        .deposit()
        .from(trader.address())
        .value(20u64.eth())
        .send_and_watch()
        .await
        .unwrap();

    onchain
        .contracts()
        .weth
        .approve(onchain.contracts().allowance, U256::MAX)
        .from(trader.address())
        .send_and_watch()
        .await
        .unwrap();

    token
        .approve(onchain.contracts().allowance, U256::MAX)
        .from(solver.address())
        .send_and_watch()
        .await
        .unwrap();

    let services = Services::new(&onchain).await;

    let mock_solver = Mock::default();

    // Start system
    colocation::start_driver(
        onchain.contracts(),
        vec![
            colocation::start_baseline_solver(
                "test_solver".into(),
                solver.clone(),
                *onchain.contracts().weth.address(),
                vec![],
                1,
                true,
            )
            .await,
            SolverEngine {
                name: "mock_solver".into(),
                account: solver.clone(),
                endpoint: mock_solver.url.clone(),
                base_tokens: vec![*token.address()],
                merge_solutions: true,
                haircut_bps: 0,
            },
        ],
        colocation::LiquidityProvider::UniswapV2,
        false,
    );

    // We start the quoter as the baseline solver, and the mock solver as the one
    // returning the solution

    let config_file = Configuration {
        drivers: vec![Solver::new(
            "mock_solver".to_string(),
            Url::from_str("http://localhost:11088/mock_solver").unwrap(),
            Account::Address(solver.address()),
        )],
    }
    .to_temp_path();

    services
        .start_autopilot(
            None,
            vec![
                format!("--config={}", config_file.path().display()),
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
        sell_token: *onchain.contracts().weth.address(),
        sell_amount: 10u64.eth(),
        buy_token: *token.address(),
        buy_amount: 5u64.eth(),
        valid_to: model::time::now_in_epoch_seconds() + 300,
        kind: OrderKind::Sell,
        ..Default::default()
    }
    .sign(
        EcdsaSigningScheme::Eip712,
        &onchain.contracts().domain_separator,
        &trader.signer,
    );

    let trader_balance_before = token.balanceOf(trader.address()).call().await.unwrap();
    let solver_balance_before = token.balanceOf(solver.address()).call().await.unwrap();
    let order_id = services.create_order(&order).await.unwrap();
    let limit_order = services.get_order(&order_id).await.unwrap();
    onchain.mint_block().await;
    assert_eq!(limit_order.metadata.class, OrderClass::Limit);

    let (jit_order, jit_order_uid) = JitOrder {
        owner: trader.address(),
        sell: Asset {
            amount: 10u64.eth(),
            token: *token.address(),
        },
        buy: Asset {
            amount: 1u64.eth(),
            token: *onchain.contracts().weth.address(),
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
        &solver.signer,
    );

    mock_solver.configure_solution(Some(Solution {
        id: 0,
        prices: HashMap::from([
            (*token.address(), 1u64.eth()),
            (*onchain.contracts().weth.address(), 1u64.eth()),
        ]),
        trades: vec![
            solvers_dto::solution::Trade::Jit(solvers_dto::solution::JitTrade {
                order: jit_order,
                // Making it 9 + 1 so we cover the edge case of fill-or-kill solution mismatches
                // when observing settlements https://github.com/cowprotocol/services/pull/3440
                executed_amount: 9u64.eth(),
                fee: Some(1u64.eth()),
            }),
            solvers_dto::solution::Trade::Fulfillment(solvers_dto::solution::Fulfillment {
                executed_amount: order.sell_amount,
                fee: Some(::alloy::primitives::U256::ZERO),
                order: solvers_dto::solution::OrderUid(order_id.0),
            }),
        ],
        pre_interactions: vec![],
        interactions: vec![],
        post_interactions: vec![],
        gas: None,
        flashloans: None,
        wrappers: vec![],
    }));

    // Drive solution
    tracing::info!("Waiting for trade.");
    onchain.mint_block().await;
    wait_for_condition(TIMEOUT, || async {
        let trader_balance_after = token.balanceOf(trader.address()).call().await.unwrap();
        let solver_balance_after = token.balanceOf(solver.address()).call().await.unwrap();

        let trader_balance_increased =
            trader_balance_after.saturating_sub(trader_balance_before) >= 5u64.eth();
        // Since the fee is 0 in the custom solution, the balance difference has to be
        // exactly 10 wei
        let solver_balance_decreased =
            solver_balance_before.saturating_sub(solver_balance_after) == 10u64.eth();
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
