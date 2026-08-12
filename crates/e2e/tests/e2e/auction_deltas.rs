use {
    ::alloy::primitives::U256,
    configs::{
        autopilot::{Configuration, run_loop::RunLoopConfig, solver::Solver},
        test_util::TestDefault,
    },
    e2e::setup::*,
    ethrpc::alloy::CallBuilderExt,
    model::{
        order::{OrderCreation, OrderKind},
        signature::EcdsaSigningScheme,
    },
    number::units::EthUnit,
    reqwest::StatusCode,
    shared::web3::Web3,
    std::time::Duration,
};

/// Port the autopilot serves its metrics on. Same as the default, spelled
/// out because the assertions below read from it.
const AUTOPILOT_METRICS_PORT: u16 = 9589;

/// A full auction every couple of seconds. The test's auctions are cut much
/// faster than that, so a single run observes both checkpoints and the deltas
/// in between them.
const CHECKPOINT_INTERVAL: Duration = Duration::from_secs(2);

/// How many unfillable orders are parked in the auction, and how much each of
/// them sells. Enough of them that the orders which do settle stay a minority
/// of the auction, little enough that they all stay covered by the balance.
const BALLAST_ORDERS: u32 = 6;
const BALLAST_SELL_AMOUNT: U256 = U256::from_limbs([10_000_000_000_000_000, 0, 0, 0]);

#[tokio::test]
#[ignore]
async fn local_node_incremental_auctions_settle_orders() {
    run_test(incremental_auctions_settle_orders).await;
}

#[tokio::test]
#[ignore]
async fn local_node_unknown_delta_base_is_rejected() {
    run_test(unknown_delta_base_is_rejected).await;
}

/// Trades two orders in a row against a driver that opted into incremental
/// auctions. The orders must settle exactly as they do with full auctions,
/// and the metrics must show that deltas were actually sent and that the
/// driver never had to be re-sent a full auction.
async fn incremental_auctions_settle_orders(web3: Web3) {
    tracing::info!("Setting up chain state.");
    let mut onchain = OnchainComponents::deploy(web3).await;

    let [solver] = onchain.make_solvers(10u64.eth()).await;
    let [trader] = onchain.make_accounts(10u64.eth()).await;
    let [token] = onchain
        .deploy_tokens_with_weth_uni_v2_pools(1_000u64.eth(), 1_000u64.eth())
        .await;

    onchain
        .contracts()
        .weth
        .approve(onchain.contracts().allowance, 6u64.eth())
        .from(trader.address())
        .send_and_watch()
        .await
        .unwrap();
    onchain
        .contracts()
        .weth
        .deposit()
        .from(trader.address())
        .value(6u64.eth())
        .send_and_watch()
        .await
        .unwrap();

    tracing::info!("Starting services with incremental auctions enabled.");
    let services = Services::new(&onchain).await;
    services
        .start_protocol_with_args(
            autopilot_config_with_deltas(&solver),
            configs::orderbook::Configuration::test_default(),
            solver.clone(),
        )
        .await;

    let place_order = async |sell_amount: U256, buy_amount: U256, kind, valid_for: u32| {
        let order = OrderCreation {
            sell_token: *onchain.contracts().weth.address(),
            sell_amount,
            buy_token: *token.address(),
            buy_amount,
            valid_to: model::time::now_in_epoch_seconds() + valid_for,
            kind,
            ..Default::default()
        }
        .sign(
            EcdsaSigningScheme::Eip712,
            &onchain.contracts().domain_separator,
            &trader.signer,
        );
        services.create_order(&order).await.unwrap()
    };
    let traded = async |expected: U256| {
        // The local node only mines on demand, so drive it forward while
        // waiting; the run loop needs new blocks to produce auctions.
        onchain.mint_block().await;
        let balance = token.balanceOf(trader.address()).call().await.unwrap();
        balance >= expected
    };

    // Orders priced far away from the market, so that no solver can ever fill
    // them. They stay in the auction unchanged from one auction to the next,
    // which is what a delta request is for: a delta is only worth sending
    // when most of the auction carries over, so an auction holding a single
    // order is always sent in full.
    tracing::info!("Placing unfillable orders to give the auction a stable body.");
    for i in 0..BALLAST_ORDERS {
        place_order(BALLAST_SELL_AMOUNT, 10u64.eth(), OrderKind::Sell, 300 + i).await;
    }

    // The first auction is always a full one, so the first order is settled
    // off a full auction. Placing the second order only afterwards makes sure
    // it reaches the driver through a delta: by then the base auction is
    // established, and the order shows up as an added order while the settled
    // one shows up as a removed one.
    tracing::info!("Placing first order.");
    place_order(2u64.eth(), 1u64.eth(), OrderKind::Buy, 900).await;
    wait_for_condition(TIMEOUT, || traded(1u64.eth()))
        .await
        .unwrap();

    tracing::info!("Placing second order.");
    place_order(2u64.eth(), 1u64.eth(), OrderKind::Buy, 1200).await;
    wait_for_condition(TIMEOUT, || traded(2u64.eth()))
        .await
        .unwrap();

    // Both orders settled; now check they did so off delta requests rather
    // than the test silently having fallen back to full auctions.
    let auctions = metric_sum("runloop_solve_request_body_size_count").await;
    let deltas = metric_sum("runloop_solve_request_delta_body_size_count").await;
    let fallbacks = metric_sum("runloop_delta_request_fallbacks").await;

    assert!(deltas > 0.0, "no delta request was ever sent");
    // The driver only starts retaining a base auction once it has seen a
    // delta, so the first delta is always rejected and re-sent in full. That
    // primes the driver; every delta after it must be accepted.
    assert_eq!(
        fallbacks, 1.0,
        "expected only the priming round to fall back, got {fallbacks}"
    );
    // A checkpoint falls due every couple of seconds, and settling two orders
    // takes considerably longer, so the run must contain further checkpoints
    // besides the initial auction.
    assert!(
        auctions - deltas >= 2.0,
        "expected checkpoints among {auctions} auctions, only {deltas} were deltas"
    );
}

/// A delta whose base auction the driver never received must be rejected
/// with a distinguishable status, since that is what tells the autopilot to
/// re-send the full auction.
async fn unknown_delta_base_is_rejected(web3: Web3) {
    let mut onchain = OnchainComponents::deploy(web3).await;
    let [solver] = onchain.make_solvers(10u64.eth()).await;
    let [_token] = onchain
        .deploy_tokens_with_weth_uni_v2_pools(1_000u64.eth(), 1_000u64.eth())
        .await;

    let services = Services::new(&onchain).await;
    services.start_protocol(solver).await;

    let delta = serde_json::json!({
        "kind": "delta",
        "id": "999999",
        // An auction the driver cannot possibly have seen.
        "baseId": "999998",
        "tokens": [],
        "updatedOrders": [],
        "removedOrders": [],
        "deadline": chrono::Utc::now() + chrono::Duration::seconds(30),
        "surplusCapturingJitOrderOwners": [],
    });
    let response = reqwest::Client::new()
        .post("http://localhost:11088/test_solver/solve")
        .header("X-Auction-Id", "999999")
        .json(&delta)
        .send()
        .await
        .unwrap();

    let status = response.status();
    let body = response.text().await.unwrap();
    assert_eq!(status, StatusCode::CONFLICT, "{body}");
    assert!(body.contains("DeltaBaseMismatch"), "{body}");
}

/// The default test configuration, with the single driver opted into
/// incremental auctions.
fn autopilot_config_with_deltas(solver: &TestAccount) -> Configuration {
    let config = Configuration::test("test_solver", solver.address());
    Configuration {
        drivers: config
            .drivers
            .iter()
            .cloned()
            .map(|driver| Solver {
                supports_auction_deltas: true,
                ..driver
            })
            .collect(),
        run_loop: RunLoopConfig {
            auction_delta_checkpoint_interval: CHECKPOINT_INTERVAL,
            ..config.run_loop
        },
        ..config
    }
}

/// Sum of all samples of the Prometheus metric whose name contains `needle`,
/// as served by the autopilot. Matching on a substring keeps this independent
/// of the registry prefix and of any labels. Metrics that were never touched
/// are not exported at all, which reads as 0.
async fn metric_sum(needle: &str) -> f64 {
    let body = reqwest::get(format!("http://localhost:{AUTOPILOT_METRICS_PORT}/metrics"))
        .await
        .unwrap()
        .text()
        .await
        .unwrap();
    body.lines()
        .filter(|line| !line.starts_with('#'))
        .filter_map(|line| {
            let (name, value) = line.rsplit_once(' ')?;
            name.contains(needle).then(|| value.parse::<f64>().ok())?
        })
        .sum()
}
