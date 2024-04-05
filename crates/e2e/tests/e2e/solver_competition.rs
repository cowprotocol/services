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
async fn local_node_solver_competition() {
    run_test(solver_competition).await;
}

async fn solver_competition(web3: Web3) {
    let mut onchain = OnchainComponents::deploy(web3).await;

    let [solver] = onchain.make_solvers(to_wei(1)).await;
    let [trader] = onchain.make_accounts(to_wei(1)).await;
    let [token_a] = onchain
        .deploy_tokens_with_weth_uni_v2_pools(to_wei(1_000), to_wei(1_000))
        .await;

    // Fund trader, settlement accounts, and pool creation
    token_a.mint(trader.address(), to_wei(10)).await;
    token_a.mint(solver.address(), to_wei(1000)).await;

    // Approve GPv2 for trading
    tx!(
        trader.account(),
        token_a.approve(onchain.contracts().allowance, to_wei(100))
    );

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
                name: "solver2".into(),
                account: solver,
                endpoint: colocation::start_baseline_solver(onchain.contracts().weth.address())
                    .await,
            },
        ],
        colocation::LiquidityProvider::UniswapV2,
    );

    let services = Services::new(onchain.contracts()).await;
    services.start_autopilot(
        None,
        vec![
            "--drivers=test_solver|http://localhost:11088/test_solver,solver2|http://localhost:11088/solver2"
                .to_string(),
            "--price-estimation-drivers=test_quoter|http://localhost:11088/test_solver,solver2|http://localhost:11088/solver2".to_string(),
        ],
    ).await;
    services.start_api(vec![
        "--price-estimation-drivers=test_quoter|http://localhost:11088/test_solver,solver2|http://localhost:11088/solver2".to_string(),
    ]).await;

    // Place Order
    let order = OrderCreation {
        sell_token: token_a.address(),
        sell_amount: to_wei(10),
        buy_token: onchain.contracts().weth.address(),
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
    let uid = services.create_order(&order).await.unwrap();

    tracing::info!("waiting for trade");
    let trade_happened =
        || async { token_a.balance_of(trader.address()).call().await.unwrap() == U256::zero() };
    wait_for_condition(TIMEOUT, trade_happened).await.unwrap();

    let indexed_trades = || async {
        onchain.mint_block().await;
        match services.get_trades(&uid).await.unwrap().first() {
            Some(trade) => services
                .get_solver_competition(trade.tx_hash.unwrap())
                .await
                .is_ok(),
            None => false,
        }
    };
    wait_for_condition(TIMEOUT, indexed_trades).await.unwrap();

    let trades = services.get_trades(&uid).await.unwrap();
    let competition = services
        .get_solver_competition(trades[0].tx_hash.unwrap())
        .await
        .unwrap();

    assert!(competition.common.solutions.len() == 2);

    // Non winning candidate
    assert!(competition.common.solutions[0].ranking == 2);
    assert!(competition.common.solutions[0].call_data.is_none());

    // Winning candidate
    assert!(competition.common.solutions[1].ranking == 1);
    assert!(competition.common.solutions[1].call_data.is_some());
}
