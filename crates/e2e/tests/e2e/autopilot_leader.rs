use {
    autopilot::shutdown_controller::ShutdownController,
    e2e::setup::{
        OnchainComponents,
        Services,
        TIMEOUT,
        colocation,
        eth,
        run_test,
        to_wei,
        wait_for_condition,
    },
    ethrpc::{
        Web3,
        alloy::{
            CallBuilderExt,
            conversions::{IntoAlloy, IntoLegacy},
        },
    },
    model::order::{OrderCreation, OrderKind},
    secp256k1::SecretKey,
    std::time::Duration,
    web3::signing::SecretKeyRef,
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
    let [trader] = onchain.make_accounts(to_wei(1)).await;
    let [solver1, solver2] = onchain.make_solvers(to_wei(1)).await;
    let [token_a] = onchain
        .deploy_tokens_with_weth_uni_v2_pools(to_wei(1_000), to_wei(1_000))
        .await;

    // Fund trader, settlement accounts, and pool creation
    token_a.mint(solver1.address(), to_wei(1000)).await;
    token_a.mint(solver2.address(), to_wei(1000)).await;

    token_a.mint(trader.address(), to_wei(200)).await;

    // Approve GPv2 for trading
    token_a
        .approve(onchain.contracts().allowance.into_alloy(), eth(1000))
        .from(trader.address().into_alloy())
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

    // Configure autopilot-leader only with test_solver
    let autopilot_leader = services.start_autopilot_with_shutdown_controller(None, vec![
        format!("--drivers=test_solver|http://localhost:11088/test_solver|{}|requested-timeout-on-problems",
            const_hex::encode(solver1.address())),
        "--price-estimation-drivers=test_quoter|http://localhost:11088/test_solver".to_string(),
        "--gas-estimators=http://localhost:11088/gasprice".to_string(),
        "--metrics-address=0.0.0.0:9590".to_string(),
        "--enable-leader-lock=true".to_string(),
    ], control).await;

    // Configure autopilot-backup only with test_solver2
    let _autopilot_follower = services.start_autopilot(None, vec![
        format!("--drivers=test_solver2|http://localhost:11088/test_solver2|{}|requested-timeout-on-problems",
            const_hex::encode(solver2.address())),
        "--price-estimation-drivers=test_quoter|http://localhost:11088/test_solver2".to_string(),
        "--gas-estimators=http://localhost:11088/gasprice".to_string(),
        "--enable-leader-lock=true".to_string(),
    ]).await;

    services
        .start_api(vec![
            "--price-estimation-drivers=test_quoter|http://localhost:11088/test_solver1,test_solver2|http://localhost:11088/test_solver2".to_string(),
        ])
        .await;

    let order = || {
        OrderCreation {
            sell_token: token_a.address().into_legacy(),
            sell_amount: to_wei(10),
            buy_token: onchain.contracts().weth.address().into_legacy(),
            buy_amount: to_wei(5),
            valid_to: model::time::now_in_epoch_seconds() + 300,
            kind: OrderKind::Sell,
            ..Default::default()
        }
        .sign(
            model::signature::EcdsaSigningScheme::Eip712,
            &onchain.contracts().domain_separator,
            SecretKeyRef::from(&SecretKey::from_slice(trader.private_key()).unwrap()),
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
