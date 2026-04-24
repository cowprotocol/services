use {
    alloy::{
        primitives::{Address, aliases::U160},
        providers::Provider,
        sol_types::SolEvent,
    },
    contracts::test::{MockUniswapV3Factory, MockUniswapV3Pool},
    e2e::setup::{OnchainComponents, TIMEOUT, colocation, run_test, wait_for_condition},
    ethrpc::Web3,
    number::units::EthUnit,
    pool_indexer::config::{
        ApiConfig,
        Configuration,
        DatabaseConfig,
        FactoryConfig,
        NetworkConfig,
        NetworkName,
    },
    sqlx::{PgPool, Row},
    std::{
        net::{Ipv4Addr, SocketAddr, SocketAddrV4},
        num::NonZeroU32,
        sync::Mutex,
        time::Duration,
    },
};

// Holds the JoinHandle of any currently-running pool-indexer so we can
// abort it even if a previous test panicked before calling handle.abort().
static CURRENT_HANDLE: Mutex<Option<tokio::task::JoinHandle<()>>> = Mutex::new(None);

const POOL_INDEXER_PORT: u16 = 7778;
const POOL_INDEXER_HOST: &str = "http://127.0.0.1:7778";
const POOL_INDEXER_METRICS_PORT: u16 = 7779;
const LOCAL_DB_URL: &str = "postgresql://";

// sqrt(1) * 2^96 — valid starting price
const INITIAL_SQRT_PRICE: u128 = 79_228_162_514_264_337_593_543_950_336;

async fn clear_pool_indexer_tables(db: &PgPool) {
    sqlx::query(
        "TRUNCATE uniswap_v3_ticks, uniswap_v3_pool_states, uniswap_v3_pools, \
         pool_indexer_checkpoints",
    )
    .execute(db)
    .await
    .unwrap();
}

async fn seed_checkpoint(db: &PgPool, factory: Address, block: u64) {
    sqlx::query(
        "INSERT INTO pool_indexer_checkpoints (chain_id, contract, block_number)
         VALUES (1, $1, $2)
         ON CONFLICT (chain_id, contract) DO UPDATE SET block_number = EXCLUDED.block_number",
    )
    .bind(factory.as_slice())
    .bind(block.cast_signed())
    .execute(db)
    .await
    .unwrap();
}

/// Start the pool-indexer. Aborts any previously-running instance first
/// (handles leftover from a prior test that panicked before calling
/// `stop_pool_indexer`). `metrics_port = 0` asks the OS to pick a random
/// port; tests that need to scrape metrics should pass a fixed port.
async fn start_pool_indexer(factory: Address) {
    start_pool_indexer_at(factory, 0).await;
}

async fn start_pool_indexer_at(factory: Address, metrics_port: u16) {
    // Abort any handle left over from a previous test that panicked.
    if let Some(old) = CURRENT_HANDLE.lock().unwrap().take() {
        old.abort();
    }
    // Always wait a bit so the previous pool-indexer (if any) has time to
    // release port 7778 before we try to bind it again.
    tokio::time::sleep(Duration::from_millis(300)).await;

    let config = Configuration {
        database: DatabaseConfig {
            url: LOCAL_DB_URL.parse().unwrap(),
            max_connections: NonZeroU32::new(5).unwrap(),
        },
        networks: vec![NetworkConfig {
            name: NetworkName::new("mainnet"),
            chain_id: 1,
            rpc_url: "http://127.0.0.1:8545".parse().unwrap(),
            factories: vec![FactoryConfig {
                address: factory,
                deployment_block: 0,
            }],
            chunk_size: 1000,
            poll_interval_secs: 1,
            use_latest: true,
            subgraph_url: None,
            seed_block: None,
            fetch_concurrency: 8,
            prefetch_concurrency: 50,
        }],
        api: ApiConfig {
            bind_address: SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::LOCALHOST, POOL_INDEXER_PORT)),
        },
        metrics: pool_indexer::config::MetricsConfig {
            bind_address: SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::LOCALHOST, metrics_port)),
        },
    };
    let handle = tokio::task::spawn(pool_indexer::run(config));
    wait_for_condition(TIMEOUT, || async {
        reqwest::get(format!("{POOL_INDEXER_HOST}/health"))
            .await
            .is_ok_and(|r| r.status().is_success())
    })
    .await
    .expect("pool-indexer API did not come up");
    *CURRENT_HANDLE.lock().unwrap() = Some(handle);
}

fn stop_pool_indexer() {
    if let Some(h) = CURRENT_HANDLE.lock().unwrap().take() {
        h.abort();
    }
}

/// Create and initialise a single pool inside an already-deployed factory.
/// `fee` must be unique within the factory for token0/token1 ([1u8;20],
/// [2u8;20]).
async fn create_pool(
    factory: &MockUniswapV3Factory::Instance,
    fee: u32,
) -> (Address, MockUniswapV3Pool::Instance) {
    let provider = factory.provider();
    let token0 = Address::from([1u8; 20]);
    let token1 = Address::from([2u8; 20]);

    factory
        .createPool(token0, token1, alloy::primitives::aliases::U24::from(fee))
        .send()
        .await
        .unwrap()
        .get_receipt()
        .await
        .unwrap();

    let block = provider.get_block_number().await.unwrap();
    let logs = provider
        .get_logs(
            &alloy::rpc::types::Filter::new()
                .from_block(block)
                .to_block(block)
                .event_signature(
                    MockUniswapV3Factory::MockUniswapV3Factory::PoolCreated::SIGNATURE_HASH,
                ),
        )
        .await
        .unwrap();
    let pool_addr =
        MockUniswapV3Factory::MockUniswapV3Factory::PoolCreated::decode_log(&logs[0].inner)
            .unwrap()
            .data
            .pool;

    let pool = MockUniswapV3Pool::Instance::new(pool_addr, provider.clone());

    pool.initialize(U160::from(INITIAL_SQRT_PRICE))
        .send()
        .await
        .unwrap()
        .get_receipt()
        .await
        .unwrap();

    pool.mockMint(
        token0,
        alloy::primitives::aliases::I24::try_from(-100i32).unwrap(),
        alloy::primitives::aliases::I24::try_from(100i32).unwrap(),
        1_000_000u128,
    )
    .send()
    .await
    .unwrap()
    .get_receipt()
    .await
    .unwrap();

    (pool_addr, pool)
}

/// Deploy mock V3 contracts and set up a pool with liquidity.
/// Returns `(factory, pool_address)`.
async fn deploy_univ3(web3: &Web3) -> (MockUniswapV3Factory::Instance, Address) {
    let provider = &web3.provider;

    let factory = MockUniswapV3Factory::Instance::deploy(provider.clone())
        .await
        .unwrap();

    let (pool_addr, _pool) = create_pool(&factory, 500).await;

    (factory, pool_addr)
}

#[tokio::test]
#[ignore]
async fn local_node_pool_indexer_happy_path() {
    run_test(happy_path).await;
}

async fn happy_path(web3: Web3) {
    let db = PgPool::connect(LOCAL_DB_URL).await.unwrap();
    clear_pool_indexer_tables(&db).await;

    let (factory, pool_addr) = deploy_univ3(&web3).await;
    let factory_addr = *factory.address();
    let head = web3.provider.get_block_number().await.unwrap();

    seed_checkpoint(&db, factory_addr, 0).await;
    start_pool_indexer(factory_addr).await;

    wait_for_condition(TIMEOUT, || async {
        let resp = reqwest::get(format!(
            "{POOL_INDEXER_HOST}/api/v1/mainnet/uniswap/v3/pools"
        ))
        .await
        .ok()?;
        let body: serde_json::Value = resp.json().await.ok()?;
        Some(body["block_number"].as_u64()? >= head)
    })
    .await
    .expect("indexer did not reach head block in time");

    let resp: serde_json::Value = reqwest::get(format!(
        "{POOL_INDEXER_HOST}/api/v1/mainnet/uniswap/v3/pools"
    ))
    .await
    .unwrap()
    .json()
    .await
    .unwrap();

    let pools = resp["pools"].as_array().unwrap();
    assert!(!pools.is_empty());
    let our_pool = pools
        .iter()
        .find(|p| {
            p["id"]
                .as_str()
                .unwrap()
                .eq_ignore_ascii_case(&format!("{pool_addr:?}"))
        })
        .expect("deployed pool not found in /pools response");
    assert_eq!(our_pool["fee_tier"].as_str().unwrap(), "500");
    assert_ne!(our_pool["sqrt_price"].as_str().unwrap(), "0");

    let resp: serde_json::Value = reqwest::get(format!(
        "{POOL_INDEXER_HOST}/api/v1/mainnet/uniswap/v3/pools/{pool_addr:?}/ticks"
    ))
    .await
    .unwrap()
    .json()
    .await
    .unwrap();
    assert!(
        !resp["ticks"].as_array().unwrap().is_empty(),
        "expected ticks from Mint event"
    );

    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM uniswap_v3_pools WHERE chain_id = 1")
        .fetch_one(&db)
        .await
        .unwrap();
    assert!(count > 0);

    stop_pool_indexer();
}

#[tokio::test]
#[ignore]
async fn local_node_pool_indexer_checkpoint_resume() {
    run_test(checkpoint_resume).await;
}

async fn checkpoint_resume(web3: Web3) {
    let db = PgPool::connect(LOCAL_DB_URL).await.unwrap();
    clear_pool_indexer_tables(&db).await;

    let (factory, pool_addr) = deploy_univ3(&web3).await;
    let factory_addr = *factory.address();
    let head = web3.provider.get_block_number().await.unwrap();
    seed_checkpoint(&db, factory_addr, 0).await;

    start_pool_indexer(factory_addr).await;
    wait_for_condition(TIMEOUT, || async {
        let resp = reqwest::get(format!(
            "{POOL_INDEXER_HOST}/api/v1/mainnet/uniswap/v3/pools"
        ))
        .await
        .ok()?;
        let body: serde_json::Value = resp.json().await.ok()?;
        Some(body["block_number"].as_u64()? >= head)
    })
    .await
    .expect("first run did not reach head");

    let pool_count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM uniswap_v3_pools WHERE chain_id = 1")
            .fetch_one(&db)
            .await
            .unwrap();

    // Capture pool state after first sync for comparison after restart.
    let row = sqlx::query(
        "SELECT sqrt_price_x96::TEXT AS price, tick, liquidity::TEXT AS liq
         FROM uniswap_v3_pool_states
         WHERE chain_id = 1 AND pool_address = $1",
    )
    .bind(pool_addr.as_slice())
    .fetch_one(&db)
    .await
    .unwrap();
    let sqrt_price_before: String = row.get("price");
    let tick_before: i32 = row.get("tick");
    let liquidity_before: String = row.get("liq");

    // stop_pool_indexer aborts and clears CURRENT_HANDLE; start_pool_indexer
    // will see no old handle, so no extra sleep needed.
    stop_pool_indexer();

    start_pool_indexer(factory_addr).await;
    wait_for_condition(TIMEOUT, || async {
        let resp = reqwest::get(format!(
            "{POOL_INDEXER_HOST}/api/v1/mainnet/uniswap/v3/pools"
        ))
        .await
        .ok()?;
        let body: serde_json::Value = resp.json().await.ok()?;
        Some(body["block_number"].as_u64()? >= head)
    })
    .await
    .expect("second run did not reach head");

    let pool_count_after: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM uniswap_v3_pools WHERE chain_id = 1")
            .fetch_one(&db)
            .await
            .unwrap();

    assert_eq!(
        pool_count, pool_count_after,
        "pool count changed after restart — idempotency violation"
    );

    // State must be identical after restart — re-indexing must not corrupt values.
    let row_after = sqlx::query(
        "SELECT sqrt_price_x96::TEXT AS price, tick, liquidity::TEXT AS liq
         FROM uniswap_v3_pool_states
         WHERE chain_id = 1 AND pool_address = $1",
    )
    .bind(pool_addr.as_slice())
    .fetch_one(&db)
    .await
    .unwrap();
    assert_eq!(
        sqrt_price_before,
        row_after.get::<String, _>("price"),
        "sqrt_price changed after restart"
    );
    assert_eq!(
        tick_before,
        row_after.get::<i32, _>("tick"),
        "tick changed after restart"
    );
    assert_eq!(
        liquidity_before,
        row_after.get::<String, _>("liq"),
        "liquidity changed after restart"
    );

    let checkpoint: i64 = sqlx::query_scalar(
        "SELECT block_number FROM pool_indexer_checkpoints
         WHERE chain_id = 1 AND contract = $1",
    )
    .bind(factory_addr.as_slice())
    .fetch_one(&db)
    .await
    .unwrap();
    assert!(checkpoint as u64 >= head);

    stop_pool_indexer();
}

// ── Test 3: API error handling
// ────────────────────────────────────────────────

#[tokio::test]
#[ignore]
async fn local_node_pool_indexer_api_errors() {
    run_test(api_errors).await;
}

async fn api_errors(web3: Web3) {
    let db = PgPool::connect(LOCAL_DB_URL).await.unwrap();
    clear_pool_indexer_tables(&db).await;

    let (factory, _pool_addr) = deploy_univ3(&web3).await;
    let factory_addr = *factory.address();
    let head = web3.provider.get_block_number().await.unwrap();

    seed_checkpoint(&db, factory_addr, 0).await;
    start_pool_indexer(factory_addr).await;

    wait_for_condition(TIMEOUT, || async {
        let resp = reqwest::get(format!(
            "{POOL_INDEXER_HOST}/api/v1/mainnet/uniswap/v3/pools"
        ))
        .await
        .ok()?;
        let body: serde_json::Value = resp.json().await.ok()?;
        Some(body["block_number"].as_u64()? >= head)
    })
    .await
    .expect("indexer did not reach head");

    // Invalid address → 400.
    let status = reqwest::get(format!(
        "{POOL_INDEXER_HOST}/api/v1/mainnet/uniswap/v3/pools/not-an-address/ticks"
    ))
    .await
    .unwrap()
    .status();
    assert_eq!(u16::from(status), 400, "expected 400 for invalid address");

    // Valid but unknown address → 200 with empty ticks array.
    let unknown = Address::from([0xABu8; 20]);
    let resp: serde_json::Value = reqwest::get(format!(
        "{POOL_INDEXER_HOST}/api/v1/mainnet/uniswap/v3/pools/{unknown:?}/ticks"
    ))
    .await
    .unwrap()
    .json()
    .await
    .unwrap();
    assert_eq!(
        resp["ticks"].as_array().unwrap().len(),
        0,
        "expected empty ticks for unknown pool"
    );

    stop_pool_indexer();
}

// ── Test 4: pagination
// ────────────────────────────────────────────────────────

#[tokio::test]
#[ignore]
async fn local_node_pool_indexer_pagination() {
    run_test(pagination).await;
}

async fn pagination(web3: Web3) {
    let db = PgPool::connect(LOCAL_DB_URL).await.unwrap();
    clear_pool_indexer_tables(&db).await;

    // Deploy factory + 3 pools (different fee tiers) so pagination has >1 page
    // to traverse with limit=1.
    let (factory, _pool1) = deploy_univ3(&web3).await;
    let factory_addr = *factory.address();
    create_pool(&factory, 3000).await;
    create_pool(&factory, 10_000).await;
    let head = web3.provider.get_block_number().await.unwrap();
    seed_checkpoint(&db, factory_addr, 0).await;

    start_pool_indexer(factory_addr).await;

    wait_for_condition(TIMEOUT, || async {
        let resp = reqwest::get(format!(
            "{POOL_INDEXER_HOST}/api/v1/mainnet/uniswap/v3/pools"
        ))
        .await
        .ok()?;
        let body: serde_json::Value = resp.json().await.ok()?;
        Some(body["block_number"].as_u64()? >= head)
    })
    .await
    .expect("indexer did not reach head");

    let mut all_ids: Vec<String> = Vec::new();
    let mut cursor: Option<String> = None;

    loop {
        let url = match &cursor {
            None => format!("{POOL_INDEXER_HOST}/api/v1/mainnet/uniswap/v3/pools?limit=1"),
            Some(c) => {
                format!("{POOL_INDEXER_HOST}/api/v1/mainnet/uniswap/v3/pools?limit=1&after={c}")
            }
        };
        let resp: serde_json::Value = reqwest::get(&url).await.unwrap().json().await.unwrap();
        let pools = resp["pools"].as_array().unwrap();
        if pools.is_empty() {
            break;
        }
        for p in pools {
            all_ids.push(p["id"].as_str().unwrap().to_owned());
        }
        cursor = resp["next_cursor"].as_str().map(|s| s.to_owned());
        if cursor.is_none() {
            break;
        }
    }

    let db_count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM uniswap_v3_pools WHERE chain_id = 1")
            .fetch_one(&db)
            .await
            .unwrap();
    assert_eq!(
        i64::try_from(all_ids.len()).unwrap(),
        db_count,
        "paginated count doesn't match DB"
    );
    assert!(
        db_count >= 3,
        "expected at least 3 pools for a meaningful pagination test"
    );
    let unique: std::collections::HashSet<_> = all_ids.iter().collect();
    assert_eq!(
        unique.len(),
        all_ids.len(),
        "pagination returned duplicates"
    );

    stop_pool_indexer();
}

/// Reads the prometheus `/metrics` endpoint and extracts the request count
/// for `GET /api/v1/{network}/uniswap/v3/pools` with status 200. The metric
/// family name is `pool_indexer_api_requests` (optionally prefixed by the
/// process registry's namespace — e.g. `driver_pool_indexer_api_requests`
/// when the driver was the first to call `setup_registry_reentrant`), so we
/// substring-match on the route+status suffix rather than assume a prefix.
/// Reads the prometheus counter `api_requests{route, status="200"}` for the
/// given route template (e.g. `/api/v1/{network}/uniswap/v3/pools`). The
/// metric family name is `pool_indexer_api_requests`, optionally prefixed by
/// the process registry's namespace (e.g. `driver_pool_indexer_api_requests`
/// when the driver was the first to call `setup_registry_reentrant`), so we
/// substring-match on the family-name-plus-labels rather than assume a
/// prefix.
async fn api_requests_counter(metrics_port: u16, route: &str) -> u64 {
    let text = reqwest::get(format!("http://127.0.0.1:{metrics_port}/metrics"))
        .await
        .unwrap()
        .text()
        .await
        .unwrap();
    let needle = format!(r#"pool_indexer_api_requests{{route="{route}",status="200"}}"#);
    for line in text.lines() {
        if line.starts_with('#') {
            continue;
        }
        if let Some(idx) = line.find(&needle) {
            let after = line[idx + needle.len()..].trim();
            return after.parse().unwrap_or(0);
        }
    }
    0
}

#[tokio::test]
#[ignore]
async fn local_node_pool_indexer_driver_integration() {
    run_test(driver_integration).await;
}

/// End-to-end: pool-indexer indexes a mock V3 factory, driver starts with
/// `pool-indexer-url` pointing at the service, and we assert (via the
/// indexer's own request counters) that the driver actually fetched pools
/// AND their ticks. The ticks endpoint is the stronger signal — it only
/// fires after `UniswapV3PoolFetcher::new` has a non-empty registered-pool
/// set to pick a top-N from. A baseline solver is spun up only because the
/// driver's TOML config requires at least one `[[solver]]`.
async fn driver_integration(web3: Web3) {
    const POOLS_ROUTE: &str = "/api/v1/{network}/uniswap/v3/pools";
    const TICKS_ROUTE: &str = "/api/v1/{network}/uniswap/v3/pools/ticks";

    let db = PgPool::connect(LOCAL_DB_URL).await.unwrap();
    clear_pool_indexer_tables(&db).await;

    let mut onchain = OnchainComponents::deploy(web3.clone()).await;
    let [solver] = onchain.make_solvers(10u64.eth()).await;

    let (factory, _pool_addr) = deploy_univ3(&web3).await;
    let factory_addr = *factory.address();
    let head = web3.provider.get_block_number().await.unwrap();
    seed_checkpoint(&db, factory_addr, 0).await;

    start_pool_indexer_at(factory_addr, POOL_INDEXER_METRICS_PORT).await;

    // Wait until the indexer has both caught up to head AND surfaced the
    // seeded pool. If we only check the block number the driver could race
    // in and see an empty registered-pool set, which would never trigger a
    // ticks fetch and silently degrade the test.
    wait_for_condition(TIMEOUT, || async {
        let resp = reqwest::get(format!(
            "{POOL_INDEXER_HOST}/api/v1/mainnet/uniswap/v3/pools"
        ))
        .await
        .ok()?;
        let body: serde_json::Value = resp.json().await.ok()?;
        let at_head = body["block_number"].as_u64()? >= head;
        let has_pool = !body["pools"].as_array()?.is_empty();
        Some(at_head && has_pool)
    })
    .await
    .expect("indexer did not reach head with pool visible");

    // Capture baselines after all test-side warm-up requests so the final
    // assertions prove the bumps came from the driver, not from the polling
    // above.
    let baseline_pools = api_requests_counter(POOL_INDEXER_METRICS_PORT, POOLS_ROUTE).await;
    let baseline_ticks = api_requests_counter(POOL_INDEXER_METRICS_PORT, TICKS_ROUTE).await;

    let baseline_solver = colocation::start_baseline_solver(
        "test_solver".into(),
        solver.clone(),
        *onchain.contracts().weth.address(),
        vec![],
        1,
        true,
    )
    .await;

    // The router address is required by the `manual` variant of the
    // uniswap-v3 config but only used at settlement time — any 20-byte value
    // is fine for a pool-fetch-only integration test.
    let config_override = format!(
        r#"
[[liquidity.uniswap-v3]]
router = "0x000000000000000000000000000000000000dEaD"
pool-indexer-url = "{POOL_INDEXER_HOST}"
max-pools-to-initialize = 10
"#
    );
    let driver_handle = colocation::start_driver_with_config_override(
        onchain.contracts(),
        vec![baseline_solver],
        colocation::LiquidityProvider::UniswapV2,
        false,
        Some(&config_override),
    );

    wait_for_condition(TIMEOUT, || async {
        let pools = api_requests_counter(POOL_INDEXER_METRICS_PORT, POOLS_ROUTE).await;
        let ticks = api_requests_counter(POOL_INDEXER_METRICS_PORT, TICKS_ROUTE).await;
        pools > baseline_pools && ticks > baseline_ticks
    })
    .await
    .expect("driver did not complete pool + tick fetch from pool-indexer within timeout");

    driver_handle.abort();
    stop_pool_indexer();
}
