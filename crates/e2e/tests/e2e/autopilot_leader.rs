use {
    autopilot::shutdown_controller::ShutdownController,
    e2e::setup::{
        OnchainComponents,
        Services,
        TIMEOUT,
        colocation,
        proxy::ReverseProxy,
        run_test,
        wait_for_condition,
    },
    ethrpc::{Web3, alloy::CallBuilderExt},
    model::order::{OrderCreation, OrderKind},
    number::units::EthUnit,
    std::time::Duration,
};

#[tokio::test]
#[ignore]
async fn local_node_dual_autopilot_only_leader_produces_auctions() {
    run_test(dual_autopilot_only_leader_produces_auctions).await;
}

async fn dual_autopilot_only_leader_produces_auctions(web3: Web3) {
    // TODO: Implement test that checks auction creation frequency against db
    // to see that only one autopilot produces auctions
    let mut onchain = OnchainComponents::deploy(web3).await;
    let [trader] = onchain.make_accounts(1u64.eth()).await;
    let [solver1, solver2] = onchain.make_solvers(1u64.eth()).await;
    let [token_a] = onchain
        .deploy_tokens_with_weth_uni_v2_pools(1_000u64.eth(), 1_000u64.eth())
        .await;

    // Fund trader, settlement accounts, and pool creation
    token_a.mint(solver1.address(), 1000u64.eth()).await;
    token_a.mint(solver2.address(), 1000u64.eth()).await;

    token_a.mint(trader.address(), 200u64.eth()).await;

    // Approve GPv2 for trading
    token_a
        .approve(onchain.contracts().allowance, 1000u64.eth())
        .from(trader.address())
        .send_and_watch()
        .await
        .unwrap();

    // set up 2 solvers
    // test_solver will be used by autopilot-leader
    // test_solver2 will be used by autopilot-backup
    colocation::start_driver(
        onchain.contracts(),
        vec![
            colocation::start_baseline_solver(
                "test_solver".into(),
                solver1.clone(),
                *onchain.contracts().weth.address(),
                vec![],
                1,
                true,
            )
            .await,
            colocation::start_baseline_solver(
                "test_solver2".into(),
                solver2.clone(),
                *onchain.contracts().weth.address(),
                vec![],
                1,
                true,
            )
            .await,
        ],
        colocation::LiquidityProvider::UniswapV2,
        false,
    );

    let services = Services::new(&onchain).await;
    let (manual_shutdown, control) = ShutdownController::new_manual_shutdown();

    // Start proxy for native price API with automatic failover
    let _proxy = ReverseProxy::start(
        "0.0.0.0:9588".parse().unwrap(),
        &[
            "http://0.0.0.0:12088".parse().unwrap(), // autopilot_leader
            "http://0.0.0.0:12089".parse().unwrap(), // autopilot_follower
        ],
    );

    // Configure autopilot-leader only with test_solver
    let autopilot_leader = services.start_autopilot_with_shutdown_controller(None, vec![
        format!("--drivers=test_solver|http://localhost:11088/test_solver|{}|requested-timeout-on-problems",
            const_hex::encode(solver1.address())),
        "--price-estimation-drivers=test_quoter|http://localhost:11088/test_solver".to_string(),
        "--gas-estimators=http://localhost:11088/gasprice".to_string(),
        "--metrics-address=0.0.0.0:9590".to_string(),
        "--api-address=0.0.0.0:12088".to_string(),
        "--enable-leader-lock=true".to_string(),
    ], control).await;

    // Configure autopilot-backup only with test_solver2
    let _autopilot_follower = services.start_autopilot(None, vec![
        format!("--drivers=test_solver2|http://localhost:11088/test_solver2|{}|requested-timeout-on-problems",
            const_hex::encode(solver2.address())),
        "--price-estimation-drivers=test_quoter|http://localhost:11088/test_solver2".to_string(),
        "--gas-estimators=http://localhost:11088/gasprice".to_string(),
        "--api-address=0.0.0.0:12089".to_string(),
        "--enable-leader-lock=true".to_string(),
    ]).await;

    services
        .start_api(vec![
            "--price-estimation-drivers=test_quoter|http://localhost:11088/test_solver1,test_solver2|http://localhost:11088/test_solver2".to_string(),
            "--native-price-estimators=Forwarder|http://0.0.0.0:9588".to_string(),
        ])
        .await;

    let order = || {
        OrderCreation {
            sell_token: *token_a.address(),
            sell_amount: 10u64.eth(),
            buy_token: *onchain.contracts().weth.address(),
            buy_amount: 5u64.eth(),
            valid_to: model::time::now_in_epoch_seconds() + 300,
            kind: OrderKind::Sell,
            ..Default::default()
        }
        .sign(
            model::signature::EcdsaSigningScheme::Eip712,
            &onchain.contracts().domain_separator,
            &trader.signer,
        )
    };

    // Run 10 txs, autopilot-leader is in charge
    // - only test_solver should participate and settle
    for i in 1..=10 {
        tracing::info!("Tx with autopilot-leader {i}");
        let uid = services.create_order(&order()).await.unwrap();

        tracing::info!("waiting for trade");
        let indexed_trades = || async {
            onchain.mint_block().await;

            if let Some(trade) = services.get_trades(&uid).await.unwrap().first() {
                services
                    .get_solver_competition(trade.tx_hash.unwrap())
                    .await
                    .ok()
                    .as_ref()
                    .and_then(|competition| competition.solutions.first())
                    .map(|solution| {
                        solution.is_winner && solution.solver_address == solver1.address()
                    })
            } else {
                None
            }
        };
        wait_for_condition(TIMEOUT, indexed_trades).await.unwrap();
    }

    // Stop autopilot-leader, follower should take over
    manual_shutdown.shutdown();
    onchain.mint_block().await;
    assert!(
        tokio::time::timeout(Duration::from_secs(15), autopilot_leader)
            .await
            .is_ok()
    );

    // Run 10 txs, autopilot-backup is in charge
    // - only test_solver2 should participate and settle
    for i in 1..=10 {
        tracing::info!("Tx with autopilot-backup {i}");
        let uid = services.create_order(&order()).await.unwrap();

        tracing::info!("waiting for trade");
        let indexed_trades = || async {
            onchain.mint_block().await;

            if let Some(trade) = services.get_trades(&uid).await.unwrap().first() {
                services
                    .get_solver_competition(trade.tx_hash.unwrap())
                    .await
                    .ok()
                    .as_ref()
                    .and_then(|competition| competition.solutions.first())
                    .map(|solution| {
                        solution.is_winner && solution.solver_address == solver2.address()
                    })
            } else {
                None
            }
        };
        wait_for_condition(TIMEOUT, indexed_trades).await.unwrap();
    }
}
