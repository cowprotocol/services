use {
    e2e::setup::{OnchainComponents, Services, TIMEOUT, run_test, to_wei, wait_for_condition},
    ethrpc::Web3,
    std::time::Duration,
    tokio::time::timeout,
};

#[tokio::test]
#[ignore]
async fn local_node_autopilot_graceful_shutdown() {
    run_test(autopilot_graceful_shutdown).await;
}

#[tokio::test]
#[ignore]
async fn local_node_dual_autopilot() {
    run_test(dual_autopilot_take_over).await;
}

#[tokio::test]
#[ignore]
async fn local_node_dual_autopilot_with_full_protocol() {
    run_test(dual_autopilot_with_full_protocol).await;
}

#[tokio::test]
#[ignore]
async fn local_node_dual_autopilot_only_leader_produces_auctions() {
    run_test(dual_autopilot_only_leader_produces_auctions).await;
}

async fn autopilot_graceful_shutdown(web3: Web3) {
    let mut onchain = OnchainComponents::deploy(web3).await;
    let [solver] = onchain.make_solvers(to_wei(1)).await;
    let services = Services::new(&onchain).await;
    let (manual_shutdown, control) = autopilot::run::Control::new_manual_shutdown();

    let autopilot = services.start_autopilot_with_control(None, vec![
        format!("--drivers=test_solver|http://localhost:11088/test_solver|{}|requested-timeout-on-problems",
            hex::encode(solver.address())),
        "--price-estimation-drivers=test_quoter|http://localhost:11088/test_solver".to_string(),
        "--gas-estimators=http://localhost:11088/gasprice".to_string(),
        "--metrics-address=0.0.0.0:9590".to_string()
    ], control).await;

    services
        .start_api(vec![
            "--price-estimation-drivers=test_quoter|http://localhost:11088/test_solver".to_string(),
        ])
        .await;

    onchain.mint_block().await;
    // Get the current auction's block
    let block = services.get_auction().await.auction.block;

    manual_shutdown.shutdown();
    assert!(
        tokio::time::timeout(Duration::from_secs(15), autopilot)
            .await
            .is_ok()
    );
    onchain.mint_block().await;
    // Assert no new auction has been made
    assert!(
        wait_for_condition(TIMEOUT, || async {
            services.get_auction().await.auction.block > block
        })
        .await
        .is_err()
    );
}

async fn dual_autopilot_take_over(web3: Web3) {
    let mut onchain = OnchainComponents::deploy(web3).await;
    let [solver] = onchain.make_solvers(to_wei(1)).await;
    let services = Services::new(&onchain).await;
    let (manual_shutdown, control) = autopilot::run::Control::new_manual_shutdown();

    let autopilot_leader = services.start_autopilot_with_control(None, vec![
        format!("--drivers=test_solver|http://localhost:11088/test_solver|{}|requested-timeout-on-problems",
            hex::encode(solver.address())),
        "--price-estimation-drivers=test_quoter|http://localhost:11088/test_solver".to_string(),
        "--gas-estimators=http://localhost:11088/gasprice".to_string(),
        "--metrics-address=0.0.0.0:9590".to_string()
    ], control).await;

    let _autopilot_shadow = services.start_autopilot(None, vec![
        format!("--drivers=test_solver|http://localhost:11088/test_solver|{}|requested-timeout-on-problems",
            hex::encode(solver.address())),
        "--price-estimation-drivers=test_quoter|http://localhost:11088/test_solver".to_string(),
        "--gas-estimators=http://localhost:11088/gasprice".to_string(),
    ]).await;

    services
        .start_api(vec![
            "--price-estimation-drivers=test_quoter|http://localhost:11088/test_solver".to_string(),
        ])
        .await;

    onchain.mint_block().await;
    // Get the current auction's block
    let block = services.get_auction().await.auction.block;

    manual_shutdown.shutdown();
    assert!(
        tokio::time::timeout(Duration::from_secs(15), autopilot_leader)
            .await
            .is_ok()
    );

    onchain.mint_block().await;

    // Assert new auction are still made
    assert!(
        wait_for_condition(Duration::from_secs(15), || async {
            services.get_auction().await.auction.block > block
        })
        .await
        .is_ok()
    );
}

async fn dual_autopilot_with_full_protocol(web3: Web3) {
    let mut onchain = OnchainComponents::deploy(web3).await;
    let [solver] = onchain.make_solvers(to_wei(1)).await;
    let services = Services::new(&onchain).await;

    let (manual_shutdown, control) = autopilot::run::Control::new_manual_shutdown();
    let autopilot_leader = services.start_autopilot_with_control(None, vec![
        format!("--drivers=test_solver|http://localhost:11088/test_solver|{}|requested-timeout-on-problems",
            hex::encode(solver.address())),
        "--price-estimation-drivers=test_quoter|http://localhost:11088/test_solver".to_string(),
        "--gas-estimators=http://localhost:11088/gasprice".to_string(),
        "--metrics-address=0.0.0.0:9590".to_string()
    ], control).await;

    services.start_protocol(solver.clone()).await;

    manual_shutdown.shutdown();
    assert!(
        tokio::time::timeout(Duration::from_secs(10), autopilot_leader)
            .await
            .is_ok()
    );
}

async fn dual_autopilot_only_leader_produces_auctions(_web3: Web3) {
    // TODO: Implement test that checks auction creation frequency against db
    // to see that only one autopilot produces auctions
}
