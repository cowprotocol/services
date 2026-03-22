use {
    autopilot::shutdown_controller::ShutdownController,
    configs::{
        autopilot::Configuration,
        order_quoting::{ExternalSolver, OrderQuoting},
        test_util::TestDefault,
    },
    e2e::setup::{
        OnchainComponents,
        Services,
        TIMEOUT,
        colocation,
        run_forked_test_with_block_number,
        run_test,
        wait_for_condition,
    },
    ethrpc::alloy::CallBuilderExt,
    futures::StreamExt,
    model::{
        order::{OrderCreation, OrderKind, OrderUid},
        signature::EcdsaSigningScheme,
    },
    number::units::EthUnit,
    reqwest::StatusCode,
    serde::Deserialize,
    serde_json::Value,
    shared::web3::Web3,
    std::{
        collections::BTreeMap,
        net::SocketAddr,
        sync::{Arc, OnceLock},
    },
    tokio::sync::{Mutex, OwnedMutexGuard},
};

#[tokio::test]
#[ignore]
async fn local_node_delta_sync_snapshot_stream_resync_recovery() {
    run_test(delta_sync_snapshot_stream_resync_recovery).await;
}

#[tokio::test]
#[ignore]
async fn local_node_delta_sync_update_pipeline_integration() {
    run_test(delta_sync_update_pipeline_integration).await;
}

#[tokio::test]
#[ignore]
async fn forked_node_delta_sync_update_pipeline_integration() {
    run_forked_test_with_block_number(
        delta_sync_update_pipeline_integration,
        std::env::var("FORK_URL_MAINNET")
            .expect("FORK_URL_MAINNET must be set to run forked tests"),
        FORK_BLOCK_MAINNET,
    )
    .await;
}

/// The block number from which we will fetch state for the forked test.
const FORK_BLOCK_MAINNET: u64 = 23112197;

async fn delta_sync_snapshot_stream_resync_recovery(web3: Web3) {
    let mut onchain = OnchainComponents::deploy(web3).await;
    let [solver] = onchain.make_solvers(1u64.eth()).await;
    let [trader] = onchain.make_accounts(2u64.eth()).await;
    let [token] = onchain
        .deploy_tokens_with_weth_uni_v2_pools(1_000u64.eth(), 1_000u64.eth())
        .await;

    token.mint(trader.address(), 100u64.eth()).await;
    token
        .approve(onchain.contracts().allowance, 100u64.eth())
        .from(trader.address())
        .send_and_watch()
        .await
        .unwrap();

    let delta_listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("failed to bind delta sync listener");
    let delta_api_addr = delta_listener
        .local_addr()
        .expect("failed to read delta sync listener address");
    let delta_api_port = delta_api_addr.port();
    let delta_api_url = format!("http://{delta_api_addr}");
    let _env = DeltaEnvGuard::enable(delta_api_url.clone()).await;

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
        ],
        colocation::LiquidityProvider::UniswapV2,
        false,
    );

    let services = Services::new(&onchain).await;
    let (shutdown_before_resync, control_before_resync) = ShutdownController::new_manual_shutdown();
    let autopilot_before_resync: tokio::task::JoinHandle<()> = services
        .start_autopilot_with_shutdown_controller(
            None,
            delta_autopilot_config(solver.address(), 9589, delta_api_port),
            control_before_resync,
        )
        .await;
    services
        .start_api(configs::orderbook::Configuration {
            order_quoting: OrderQuoting::test_with_drivers(vec![ExternalSolver::new(
                "test_quoter",
                "http://localhost:11088/test_solver",
            )]),
            native_price_estimation: configs::orderbook::native_price::NativePriceConfig {
                estimators: configs::native_price_estimators::NativePriceEstimators::new(vec![
                    vec![
                        configs::native_price_estimators::NativePriceEstimator::driver(
                            "test_quoter".to_string(),
                            "http://localhost:11088/test_solver".parse().unwrap(),
                        ),
                    ],
                ]),
                ..configs::orderbook::native_price::NativePriceConfig::test_default()
            },
            ..configs::orderbook::Configuration::test_default()
        })
        .await;

    let first_order = OrderCreation {
        sell_token: *token.address(),
        sell_amount: 5u64.eth(),
        buy_token: *onchain.contracts().weth.address(),
        buy_amount: 1u64.eth(),
        valid_to: model::time::now_in_epoch_seconds() + 300,
        kind: OrderKind::Sell,
        ..Default::default()
    }
    .sign(
        EcdsaSigningScheme::Eip712,
        &onchain.contracts().domain_separator,
        &trader.signer,
    );
    let first_uid = services.create_order(&first_order).await.unwrap();
    wait_for_settlement(&onchain, &services, &first_uid).await;

    let snapshot_before_resync = wait_for_delta_snapshot(&delta_api_url).await;
    assert_eq!(snapshot_before_resync.version, 1);
    assert!(snapshot_before_resync.sequence > 0);

    // Force driver delta consumer to reconnect and resnapshot by dropping the
    // live delta stream at source (autopilot restart).
    shutdown_before_resync.shutdown();
    wait_for_condition(TIMEOUT, || async {
        onchain.mint_block().await;
        autopilot_before_resync.is_finished()
    })
    .await
    .unwrap();

    let (_shutdown_after_resync, control_after_resync) = ShutdownController::new_manual_shutdown();
    let _delta_listener =
        tokio::net::TcpListener::bind(SocketAddr::from(([127, 0, 0, 1], delta_api_port)))
            .await
            .expect("failed to bind delta sync listener for restart");
    let _autopilot_after_resync: tokio::task::JoinHandle<()> = services
        .start_autopilot_with_shutdown_controller(
            None,
            delta_autopilot_config(solver.address(), 9590, delta_api_port),
            control_after_resync,
        )
        .await;

    let second_order = OrderCreation {
        sell_token: *token.address(),
        sell_amount: 4u64.eth(),
        buy_token: *onchain.contracts().weth.address(),
        buy_amount: 1u64.eth(),
        valid_to: model::time::now_in_epoch_seconds() + 300,
        kind: OrderKind::Sell,
        ..Default::default()
    }
    .sign(
        EcdsaSigningScheme::Eip712,
        &onchain.contracts().domain_separator,
        &trader.signer,
    );
    let second_uid = services.create_order(&second_order).await.unwrap();
    wait_for_settlement(&onchain, &services, &second_uid).await;

    let snapshot_after_resync = wait_for_delta_snapshot(&delta_api_url).await;
    assert_eq!(snapshot_after_resync.version, 1);
    assert!(snapshot_after_resync.sequence > 0);
}

async fn delta_sync_update_pipeline_integration(web3: Web3) {
    let mut onchain = OnchainComponents::deploy(web3).await;
    let [solver] = onchain.make_solvers(1u64.eth()).await;
    let [trader] = onchain.make_accounts(2u64.eth()).await;
    let [token] = onchain
        .deploy_tokens_with_weth_uni_v2_pools(1_000u64.eth(), 1_000u64.eth())
        .await;

    token.mint(trader.address(), 200u64.eth()).await;
    token
        .approve(onchain.contracts().allowance, 200u64.eth())
        .from(trader.address())
        .send_and_watch()
        .await
        .unwrap();

    let delta_listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("failed to bind delta sync listener");
    let delta_api_addr = delta_listener
        .local_addr()
        .expect("failed to read delta sync listener address");
    let delta_api_url = format!("http://{delta_api_addr}");
    let _env = DeltaEnvGuard::enable(delta_api_url.clone()).await;

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
        ],
        colocation::LiquidityProvider::UniswapV2,
        false,
    );

    let services = Services::new(&onchain).await;
    let (_shutdown, control) = ShutdownController::new_manual_shutdown();
    let _autopilot: tokio::task::JoinHandle<()> = services
        .start_autopilot_with_shutdown_controller(
            None,
            delta_autopilot_config(solver.address(), 9589, delta_api_addr.port()),
            control,
        )
        .await;
    services
        .start_api(configs::orderbook::Configuration {
            order_quoting: OrderQuoting::test_with_drivers(vec![ExternalSolver::new(
                "test_quoter",
                "http://localhost:11088/test_solver",
            )]),
            native_price_estimation: configs::orderbook::native_price::NativePriceConfig {
                estimators: configs::native_price_estimators::NativePriceEstimators::new(vec![
                    vec![
                        configs::native_price_estimators::NativePriceEstimator::driver(
                            "test_quoter".to_string(),
                            "http://localhost:11088/test_solver".parse().unwrap(),
                        ),
                    ],
                ]),
                ..configs::orderbook::native_price::NativePriceConfig::test_default()
            },
            ..configs::orderbook::Configuration::test_default()
        })
        .await;

    let first_order = OrderCreation {
        sell_token: *token.address(),
        sell_amount: 5u64.eth(),
        buy_token: *onchain.contracts().weth.address(),
        buy_amount: 4u64.eth(),
        valid_to: model::time::now_in_epoch_seconds() + 600,
        kind: OrderKind::Sell,
        ..Default::default()
    }
    .sign(
        EcdsaSigningScheme::Eip712,
        &onchain.contracts().domain_separator,
        &trader.signer,
    );
    let first_uid = services.create_order(&first_order).await.unwrap();
    onchain.mint_block().await;

    let first_snapshot = wait_for_snapshot_with_order(&delta_api_url, &first_uid).await;
    assert_eq!(first_snapshot.version, 1);
    let first_sequence = first_snapshot.sequence;
    assert_snapshot_replay_matches(&delta_api_url, first_snapshot).await;

    let second_order = OrderCreation {
        sell_token: *token.address(),
        sell_amount: 7u64.eth(),
        buy_token: *onchain.contracts().weth.address(),
        buy_amount: 6u64.eth(),
        valid_to: model::time::now_in_epoch_seconds() + 600,
        kind: OrderKind::Sell,
        ..Default::default()
    }
    .sign(
        EcdsaSigningScheme::Eip712,
        &onchain.contracts().domain_separator,
        &trader.signer,
    );
    let second_uid = services.create_order(&second_order).await.unwrap();
    onchain.mint_block().await;

    wait_for_delta_event_with_order(&delta_api_url, first_sequence, &second_uid).await;
}

async fn wait_for_settlement(onchain: &OnchainComponents, services: &Services<'_>, uid: &OrderUid) {
    wait_for_condition(TIMEOUT, || async {
        onchain.mint_block().await;
        services
            .get_trades(uid)
            .await
            .ok()
            .is_some_and(|trades| !trades.is_empty())
    })
    .await
    .unwrap();
}

fn delta_autopilot_config(
    solver_address: alloy::primitives::Address,
    metrics_port: u16,
    api_port: u16,
) -> Configuration {
    let mut config = Configuration {
        order_quoting: OrderQuoting::test_with_drivers(vec![ExternalSolver::new(
            "test_quoter",
            "http://localhost:11088/test_solver",
        )]),
        ..Configuration::test("test_solver", solver_address)
    };
    config.metrics_address = SocketAddr::from(([127, 0, 0, 1], metrics_port));
    config.api_address = SocketAddr::from(([127, 0, 0, 1], api_port));
    config
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct DeltaSnapshotDto {
    version: u32,
    sequence: u64,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct DeltaSnapshotFullDto {
    version: u32,
    sequence: u64,
    oldest_available: u64,
    auction: Value,
}

async fn wait_for_delta_snapshot(delta_api_url: &str) -> DeltaSnapshotDto {
    let client = reqwest::Client::new();
    let snapshot = std::sync::Arc::new(tokio::sync::Mutex::new(None));

    wait_for_condition(TIMEOUT, || {
        let client = client.clone();
        let snapshot = std::sync::Arc::clone(&snapshot);
        async move {
            let response = client
                .get(format!("{delta_api_url}/delta/snapshot"))
                .send()
                .await;
            let Ok(response) = response else {
                return false;
            };
            if response.status() != StatusCode::OK {
                return false;
            }
            let Ok(body) = response.json::<DeltaSnapshotDto>().await else {
                return false;
            };
            *snapshot.lock().await = Some(body);
            true
        }
    })
    .await
    .expect("delta snapshot did not become available");

    snapshot
        .lock()
        .await
        .take()
        .expect("delta snapshot payload missing")
}

async fn wait_for_snapshot_with_order(delta_api_url: &str, uid: &OrderUid) -> DeltaSnapshotFullDto {
    let client = reqwest::Client::new();
    let snapshot = std::sync::Arc::new(tokio::sync::Mutex::new(None));
    let uid = uid.to_string();

    wait_for_condition(TIMEOUT, || {
        let client = client.clone();
        let snapshot = std::sync::Arc::clone(&snapshot);
        let uid = uid.clone();
        async move {
            let response = client
                .get(format!("{delta_api_url}/delta/snapshot"))
                .send()
                .await;
            let Ok(response) = response else {
                return false;
            };
            if response.status() != StatusCode::OK {
                return false;
            }
            let Ok(body) = response.json::<DeltaSnapshotFullDto>().await else {
                return false;
            };
            let has_order = auction_orders(&body.auction)
                .keys()
                .any(|order_uid| order_uid == &uid);
            if has_order {
                *snapshot.lock().await = Some(body);
                true
            } else {
                false
            }
        }
    })
    .await
    .expect("snapshot did not include expected order");

    snapshot
        .lock()
        .await
        .take()
        .expect("delta snapshot payload missing")
}

async fn wait_for_delta_snapshot_full(delta_api_url: &str) -> DeltaSnapshotFullDto {
    let client = reqwest::Client::new();
    let snapshot = std::sync::Arc::new(tokio::sync::Mutex::new(None));

    wait_for_condition(TIMEOUT, || {
        let client = client.clone();
        let snapshot = std::sync::Arc::clone(&snapshot);
        async move {
            let response = client
                .get(format!("{delta_api_url}/delta/snapshot"))
                .send()
                .await;
            let Ok(response) = response else {
                return false;
            };
            if response.status() != StatusCode::OK {
                return false;
            }
            let Ok(body) = response.json::<DeltaSnapshotFullDto>().await else {
                return false;
            };
            *snapshot.lock().await = Some(body);
            true
        }
    })
    .await
    .expect("delta snapshot did not become available");

    snapshot
        .lock()
        .await
        .take()
        .expect("delta snapshot payload missing")
}

async fn assert_snapshot_replay_matches(delta_api_url: &str, mut snapshot: DeltaSnapshotFullDto) {
    for _ in 0..3 {
        if snapshot.oldest_available == 0 {
            if let Some(replayed) =
                replay_delta_history(delta_api_url, snapshot.oldest_available, snapshot.sequence)
                    .await
            {
                let snapshot_orders = auction_orders(&snapshot.auction);
                let snapshot_prices = auction_prices(&snapshot.auction);
                assert_eq!(replayed.orders, snapshot_orders);
                assert_eq!(replayed.prices, snapshot_prices);
                return;
            }
        }
        snapshot = wait_for_delta_snapshot_full(delta_api_url).await;
    }

    panic!("delta replay did not align with snapshot sequence");
}

#[derive(Default)]
struct ReplayedAuctionState {
    orders: BTreeMap<String, Value>,
    prices: BTreeMap<String, Value>,
}

async fn replay_delta_history(
    delta_api_url: &str,
    after_sequence: u64,
    target_sequence: u64,
) -> Option<ReplayedAuctionState> {
    let client = reqwest::Client::new();
    let response = client
        .get(format!("{delta_api_url}/delta/stream"))
        .query(&[("after_sequence", after_sequence)])
        .send()
        .await
        .expect("delta stream request failed");
    assert_eq!(response.status(), StatusCode::OK);

    let mut stream = response.bytes_stream();
    let mut buffer = String::new();
    let deadline = tokio::time::Instant::now() + TIMEOUT;
    let mut state = ReplayedAuctionState::default();
    let mut last_sequence = after_sequence;

    loop {
        let next = tokio::time::timeout_at(deadline, stream.next())
            .await
            .expect("delta stream timeout");
        let Some(chunk) = next else {
            panic!("delta stream closed before expected replay");
        };
        let chunk = chunk.expect("delta stream read failed");
        let text = String::from_utf8_lossy(&chunk).replace("\r\n", "\n");
        buffer.push_str(&text);

        while let Some(idx) = buffer.find("\n\n") {
            let block = buffer[..idx].to_string();
            buffer.drain(..idx + 2);
            let (event, data) = parse_sse_block(&block);
            if event != "delta" {
                continue;
            }
            let Some(data) = data else {
                continue;
            };
            let envelope: Value = serde_json::from_str(&data).expect("delta event json");
            let to_sequence = envelope
                .get("toSequence")
                .and_then(|value| value.as_u64())
                .expect("delta event toSequence missing");
            if to_sequence <= last_sequence {
                continue;
            }

            let events = envelope
                .get("events")
                .and_then(|value| value.as_array())
                .cloned()
                .unwrap_or_default();
            for event in events {
                apply_delta_event(&mut state, &event);
            }

            last_sequence = to_sequence;
            if to_sequence == target_sequence {
                return Some(state);
            }
            if to_sequence > target_sequence {
                return None;
            }
        }
    }
}

fn apply_delta_event(state: &mut ReplayedAuctionState, event: &Value) {
    let event_type = event.get("type").and_then(|value| value.as_str());
    match event_type {
        Some("orderAdded") | Some("orderUpdated") => {
            if let Some(order) = event.get("order") {
                if let Some(uid) = order.get("uid").and_then(|value| value.as_str()) {
                    state.orders.insert(uid.to_string(), order.clone());
                }
            }
        }
        Some("orderRemoved") => {
            if let Some(uid) = event.get("uid").and_then(|value| value.as_str()) {
                state.orders.remove(uid);
            }
        }
        Some("priceChanged") => {
            let token = event.get("token").and_then(|value| value.as_str());
            let price = event.get("price");
            if let Some(token) = token {
                match price {
                    Some(Value::Null) | None => {
                        state.prices.remove(token);
                    }
                    Some(price) => {
                        state.prices.insert(token.to_string(), price.clone());
                    }
                }
            }
        }
        _ => {}
    }
}

fn auction_orders(auction: &Value) -> BTreeMap<String, Value> {
    let mut orders = BTreeMap::new();
    let Some(items) = auction.get("orders").and_then(|value| value.as_array()) else {
        return orders;
    };
    for order in items {
        if let Some(uid) = order.get("uid").and_then(|value| value.as_str()) {
            orders.insert(uid.to_string(), order.clone());
        }
    }
    orders
}

fn auction_prices(auction: &Value) -> BTreeMap<String, Value> {
    let mut prices = BTreeMap::new();
    let Some(items) = auction.get("prices").and_then(|value| value.as_object()) else {
        return prices;
    };
    for (token, price) in items {
        prices.insert(token.clone(), price.clone());
    }
    prices
}

async fn wait_for_delta_event_with_order(delta_api_url: &str, after_sequence: u64, uid: &OrderUid) {
    let client = reqwest::Client::new();
    let response = client
        .get(format!("{delta_api_url}/delta/stream"))
        .query(&[("after_sequence", after_sequence)])
        .send()
        .await
        .expect("delta stream request failed");
    assert_eq!(response.status(), StatusCode::OK);

    let uid = uid.to_string();
    let mut stream = response.bytes_stream();
    let mut buffer = String::new();
    let deadline = tokio::time::Instant::now() + TIMEOUT;

    loop {
        let next = tokio::time::timeout_at(deadline, stream.next())
            .await
            .expect("delta stream timeout");
        let Some(chunk) = next else {
            panic!("delta stream closed before expected event");
        };
        let chunk = chunk.expect("delta stream read failed");
        let text = String::from_utf8_lossy(&chunk).replace("\r\n", "\n");
        buffer.push_str(&text);

        while let Some(idx) = buffer.find("\n\n") {
            let block = buffer[..idx].to_string();
            buffer.drain(..idx + 2);
            let (event, data) = parse_sse_block(&block);
            if event != "delta" {
                continue;
            }
            let Some(data) = data else {
                continue;
            };
            let envelope: Value = serde_json::from_str(&data).expect("delta event json");
            let events = envelope
                .get("events")
                .and_then(|value| value.as_array())
                .cloned()
                .unwrap_or_default();
            for event in events {
                let event_type = event.get("type").and_then(|value| value.as_str());
                let matches_uid = match event_type {
                    Some("orderAdded") | Some("orderUpdated") => event
                        .get("order")
                        .and_then(|order| order.get("uid"))
                        .and_then(|value| value.as_str())
                        .is_some_and(|value| value == uid),
                    Some("orderRemoved") => event
                        .get("uid")
                        .and_then(|value| value.as_str())
                        .is_some_and(|value| value == uid),
                    _ => false,
                };
                if matches_uid {
                    return;
                }
            }
        }
    }
}

fn parse_sse_block(block: &str) -> (&str, Option<String>) {
    let mut event = "message";
    let mut data_lines = Vec::new();

    for raw_line in block.lines() {
        if raw_line.is_empty() || raw_line.starts_with(':') {
            continue;
        }

        let (field, value) = raw_line
            .split_once(':')
            .map(|(field, value)| (field, value.strip_prefix(' ').unwrap_or(value)))
            .unwrap_or((raw_line, ""));

        match field {
            "event" => event = value,
            "data" => data_lines.push(value),
            _ => {}
        }
    }

    if data_lines.is_empty() {
        (event, None)
    } else {
        (event, Some(data_lines.join("\n")))
    }
}

static DELTA_ENV_MUTEX: OnceLock<Arc<Mutex<()>>> = OnceLock::new();

struct DeltaEnvGuard {
    previous: Vec<(&'static str, Option<String>)>,
    _lock: OwnedMutexGuard<()>,
}

impl DeltaEnvGuard {
    async fn enable(delta_api_url: String) -> Self {
        let lock = DELTA_ENV_MUTEX
            .get_or_init(|| Arc::new(Mutex::new(())))
            .clone()
            .lock_owned()
            .await;
        let keys = [
            "AUTOPILOT_DELTA_SYNC_ENABLED",
            "DRIVER_DELTA_SYNC_ENABLED",
            "DRIVER_DELTA_SYNC_USE_REPLICA",
            "DRIVER_DELTA_SYNC_AUTOPILOT_URL",
        ];

        let previous = keys
            .iter()
            .map(|key| (*key, std::env::var(key).ok()))
            .collect::<Vec<_>>();

        unsafe {
            std::env::set_var("AUTOPILOT_DELTA_SYNC_ENABLED", "true");
            std::env::set_var("DRIVER_DELTA_SYNC_ENABLED", "true");
            std::env::set_var("DRIVER_DELTA_SYNC_USE_REPLICA", "true");
            std::env::set_var("DRIVER_DELTA_SYNC_AUTOPILOT_URL", delta_api_url);
        }

        Self {
            previous,
            _lock: lock,
        }
    }
}

impl Drop for DeltaEnvGuard {
    fn drop(&mut self) {
        for (key, value) in &self.previous {
            match value {
                Some(value) => unsafe {
                    std::env::set_var(key, value);
                },
                None => unsafe {
                    std::env::remove_var(key);
                },
            }
        }
    }
}
