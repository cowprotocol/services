//! End-to-end check that the driver consumes pool data from `pool-indexer`
//! when `pool-indexer-url` is set. Pre-seeds the indexer checkpoint so the
//! subgraph_seeder bootstrap is skipped (Anvil has no subgraph); only the
//! live-indexing and HTTP-serving paths are exercised.

use {
    alloy::{
        primitives::{
            Address,
            aliases::{I24, U24, U160},
        },
        providers::Provider,
        sol,
        sol_types::SolEvent,
    },
    e2e::setup::{OnchainComponents, TIMEOUT, colocation, run_test, wait_for_condition},
    ethrpc::Web3,
    number::units::EthUnit,
    pool_indexer::config::{
        ApiConfig,
        Configuration,
        DatabaseConfig,
        FactoryConfig,
        MetricsConfig,
        NetworkConfig,
        NetworkName,
    },
    serde::Deserialize,
    sqlx::PgPool,
    std::{
        future::Future,
        net::{Ipv4Addr, SocketAddr, SocketAddrV4},
        num::NonZeroU32,
    },
};

// Mock V3 factory. Bytecode compiled from the .sol source below with solc
// 0.8.30 --optimize --optimize-runs 1000000, evm-version shanghai.
//
// // SPDX-License-Identifier: GPL-3.0-or-later
// pragma solidity ^0.8.17;
// import "./MockUniswapV3Pool.sol";
// contract MockUniswapV3Factory {
//     event PoolCreated(
//         address indexed token0, address indexed token1, uint24 indexed fee,
//         int24 tickSpacing, address pool
//     );
//     function createPool(address tokenA, address tokenB, uint24 _fee)
//         external returns (address pool)
//     {
//         (address t0, address t1) =
//             tokenA < tokenB ? (tokenA, tokenB) : (tokenB, tokenA);
//         MockUniswapV3Pool p = new MockUniswapV3Pool(t0, t1, _fee);
//         pool = address(p);
//         emit PoolCreated(t0, t1, _fee, int24(10), pool);
//     }
// }
sol! {
    #[allow(missing_docs)]
    #[sol(rpc, bytecode = "0x6080604052348015600e575f5ffd5b506106dd8061001c5f395ff3fe608060405234801561000f575f5ffd5b5060043610610029575f3560e01c8063a16712951461002d575b5f5ffd5b61004061003b3660046101ab565b610069565b60405173ffffffffffffffffffffffffffffffffffffffff909116815260200160405180910390f35b5f5f5f8473ffffffffffffffffffffffffffffffffffffffff168673ffffffffffffffffffffffffffffffffffffffff16106100a65784866100a9565b85855b915091505f8282866040516100bd90610176565b73ffffffffffffffffffffffffffffffffffffffff938416815292909116602083015262ffffff166040820152606001604051809103905ff080158015610106573d5f5f3e3d5ffd5b5060408051600a815273ffffffffffffffffffffffffffffffffffffffff808416602083015292965086935062ffffff88169280861692908716917f783cca1c0412dd0d695e784568c96da2e9c22ff989357a2e8b1d9b2b4e6b7118910160405180910390a45050509392505050565b6104da806101f783390190565b803573ffffffffffffffffffffffffffffffffffffffff811681146101a6575f5ffd5b919050565b5f5f5f606084860312156101bd575f5ffd5b6101c684610183565b92506101d460208501610183565b9150604084013562ffffff811681146101eb575f5ffd5b80915050925092509256fe60e060405234801561000f575f5ffd5b506040516104da3803806104da83398101604081905261002e91610069565b6001600160a01b03928316608052911660a05262ffffff1660c0526100b4565b80516001600160a01b0381168114610064575f5ffd5b919050565b5f5f5f6060848603121561007b575f5ffd5b6100848461004e565b92506100926020850161004e565b9150604084015162ffffff811681146100a9575f5ffd5b809150509250925092565b60805160a05160c0516103fd6100dd5f395f61012c01525f61010501525f607801526103fd5ff3fe608060405234801561000f575f5ffd5b506004361061006f575f3560e01c8063ddca3f431161004d578063ddca3f4314610127578063efe27fa314610162578063f637731d14610177575f5ffd5b80630dfe1681146100735780631a686502146100c4578063d21220a714610100575b5f5ffd5b61009a7f000000000000000000000000000000000000000000000000000000000000000081565b60405173ffffffffffffffffffffffffffffffffffffffff90911681526020015b60405180910390f35b5f546100df906fffffffffffffffffffffffffffffffff1681565b6040516fffffffffffffffffffffffffffffffff90911681526020016100bb565b61009a7f000000000000000000000000000000000000000000000000000000000000000081565b61014e7f000000000000000000000000000000000000000000000000000000000000000081565b60405162ffffff90911681526020016100bb565b610175610170366004610312565b61018a565b005b61017561018536600461037b565b610287565b5f805482919081906101af9084906fffffffffffffffffffffffffffffffff1661039d565b92506101000a8154816fffffffffffffffffffffffffffffffff02191690836fffffffffffffffffffffffffffffffff1602179055508160020b8360020b8573ffffffffffffffffffffffffffffffffffffffff167f7a53080ba414158be7ec69b987b5fb7d07dee101fe85488f0853ae16239d0bde33855f5f604051610279949392919073ffffffffffffffffffffffffffffffffffffffff9490941684526fffffffffffffffffffffffffffffffff9290921660208401526040830152606082015260800190565b60405180910390a450505050565b6040805173ffffffffffffffffffffffffffffffffffffffff831681525f60208201527f98636036cb66a9c19a37435efc1e90142190214e8abeb821bdba3f2990dd4c95910160405180910390a150565b73ffffffffffffffffffffffffffffffffffffffff811681146102f9575f5ffd5b50565b8035600281900b811461030d575f5ffd5b919050565b5f5f5f5f60808587031215610325575f5ffd5b8435610330816102d8565b935061033e602086016102fc565b925061034c604086016102fc565b915060608501356fffffffffffffffffffffffffffffffff81168114610370575f5ffd5b939692955090935050565b5f6020828403121561038b575f5ffd5b8135610396816102d8565b9392505050565b6fffffffffffffffffffffffffffffffff81811683821601908111156103ea577f4e487b71000000000000000000000000000000000000000000000000000000005f52601160045260245ffd5b9291505056fea164736f6c634300081e000aa164736f6c634300081e000a")]
    contract MockUniswapV3Factory {
        event PoolCreated(
            address indexed token0,
            address indexed token1,
            uint24  indexed fee,
            int24           tickSpacing,
            address         pool
        );

        function createPool(
            address tokenA,
            address tokenB,
            uint24  _fee
        ) external returns (address pool);
    }
}

// Mock V3 pool. Compiled identically to the factory above.
//
// // SPDX-License-Identifier: GPL-3.0-or-later
// pragma solidity ^0.8.17;
// contract MockUniswapV3Pool {
//     address public immutable token0;
//     address public immutable token1;
//     uint24  public immutable fee;
//     uint128 public liquidity;
//     event Initialize(uint160 sqrtPriceX96, int24 tick);
//     event Mint(
//         address sender, address indexed owner,
//         int24 indexed tickLower, int24 indexed tickUpper,
//         uint128 amount, uint256 amount0, uint256 amount1
//     );
//     constructor(address _token0, address _token1, uint24 _fee) {
//         token0 = _token0; token1 = _token1; fee = _fee;
//     }
//     function initialize(uint160 sqrtPriceX96) external {
//         emit Initialize(sqrtPriceX96, int24(0));
//     }
//     function mockMint(
//         address owner, int24 tickLower, int24 tickUpper, uint128 amount
//     ) external {
//         liquidity += amount;
//         emit Mint(msg.sender, owner, tickLower, tickUpper, amount, 0, 0);
//     }
// }
sol! {
    #[allow(missing_docs)]
    #[sol(rpc, bytecode = "0x60e060405234801561000f575f5ffd5b506040516104da3803806104da83398101604081905261002e91610069565b6001600160a01b03928316608052911660a05262ffffff1660c0526100b4565b80516001600160a01b0381168114610064575f5ffd5b919050565b5f5f5f6060848603121561007b575f5ffd5b6100848461004e565b92506100926020850161004e565b9150604084015162ffffff811681146100a9575f5ffd5b809150509250925092565b60805160a05160c0516103fd6100dd5f395f61012c01525f61010501525f607801526103fd5ff3fe608060405234801561000f575f5ffd5b506004361061006f575f3560e01c8063ddca3f431161004d578063ddca3f4314610127578063efe27fa314610162578063f637731d14610177575f5ffd5b80630dfe1681146100735780631a686502146100c4578063d21220a714610100575b5f5ffd5b61009a7f000000000000000000000000000000000000000000000000000000000000000081565b60405173ffffffffffffffffffffffffffffffffffffffff90911681526020015b60405180910390f35b5f546100df906fffffffffffffffffffffffffffffffff1681565b6040516fffffffffffffffffffffffffffffffff90911681526020016100bb565b61009a7f000000000000000000000000000000000000000000000000000000000000000081565b61014e7f000000000000000000000000000000000000000000000000000000000000000081565b60405162ffffff90911681526020016100bb565b610175610170366004610312565b61018a565b005b61017561018536600461037b565b610287565b5f805482919081906101af9084906fffffffffffffffffffffffffffffffff1661039d565b92506101000a8154816fffffffffffffffffffffffffffffffff02191690836fffffffffffffffffffffffffffffffff1602179055508160020b8360020b8573ffffffffffffffffffffffffffffffffffffffff167f7a53080ba414158be7ec69b987b5fb7d07dee101fe85488f0853ae16239d0bde33855f5f604051610279949392919073ffffffffffffffffffffffffffffffffffffffff9490941684526fffffffffffffffffffffffffffffffff9290921660208401526040830152606082015260800190565b60405180910390a450505050565b6040805173ffffffffffffffffffffffffffffffffffffffff831681525f60208201527f98636036cb66a9c19a37435efc1e90142190214e8abeb821bdba3f2990dd4c95910160405180910390a150565b73ffffffffffffffffffffffffffffffffffffffff811681146102f9575f5ffd5b50565b8035600281900b811461030d575f5ffd5b919050565b5f5f5f5f60808587031215610325575f5ffd5b8435610330816102d8565b935061033e602086016102fc565b925061034c604086016102fc565b915060608501356fffffffffffffffffffffffffffffffff81168114610370575f5ffd5b939692955090935050565b5f6020828403121561038b575f5ffd5b8135610396816102d8565b9392505050565b6fffffffffffffffffffffffffffffffff81811683821601908111156103ea577f4e487b71000000000000000000000000000000000000000000000000000000005f52601160045260245ffd5b9291505056fea164736f6c634300081e000a")]
    contract MockUniswapV3Pool {
        event Initialize(uint160 sqrtPriceX96, int24 tick);
        event Mint(
            address          sender,
            address indexed  owner,
            int24   indexed  tickLower,
            int24   indexed  tickUpper,
            uint128          amount,
            uint256          amount0,
            uint256          amount1
        );

        function initialize(uint160 sqrtPriceX96) external;
        function mockMint(
            address owner,
            int24   tickLower,
            int24   tickUpper,
            uint128 amount
        ) external;
    }
}

const POOL_INDEXER_PORT: u16 = 7778;
const POOL_INDEXER_HOST: &str = "http://127.0.0.1:7778";
const POOL_INDEXER_METRICS_PORT: u16 = 7779;
const LOCAL_DB_URL: &str = "postgresql://";

// sqrt(1) * 2^96 — valid starting price
const INITIAL_SQRT_PRICE: u128 = 1u128 << 96;

// Never queried — the pre-seeded checkpoint short-circuits the seeder.
const PLACEHOLDER_SUBGRAPH_URL: &str = "http://127.0.0.1:1/never-queried";

/// Typed shape of `GET /api/v1/{network}/uniswap/v3/pools`.
#[derive(Deserialize)]
struct PoolsListResponse {
    block_number: u64,
    pools: Vec<PoolEntry>,
    #[serde(default)]
    next_cursor: Option<String>,
}

#[derive(Deserialize)]
struct PoolEntry {
    id: String,
}

/// Typed shape of `GET /api/v1/{network}/uniswap/v3/pools/{pool}/ticks`.
#[derive(Deserialize)]
struct TicksResponse {
    ticks: Vec<TickEntry>,
}

#[derive(Deserialize)]
struct TickEntry {}

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
        "INSERT INTO pool_indexer_checkpoints (contract_address, block_number)
         VALUES ($1, $2)
         ON CONFLICT (contract_address) DO UPDATE SET block_number = EXCLUDED.block_number",
    )
    .bind(factory.as_slice())
    .bind(block.cast_signed())
    .execute(db)
    .await
    .unwrap();
}

/// Spawns the pool-indexer task and waits for its `/health` endpoint to come
/// up. Used by `with_pool_indexer` and `with_pool_indexer_at`.
async fn spawn_pool_indexer(factory: Address, metrics_port: u16) -> tokio::task::JoinHandle<()> {
    let config = Configuration {
        database: DatabaseConfig {
            url: LOCAL_DB_URL.parse().unwrap(),
            max_connections: NonZeroU32::new(5).unwrap(),
        },
        network: NetworkConfig {
            name: NetworkName::new("mainnet"),
            chain_id: 1,
            rpc_url: "http://127.0.0.1:8545".parse().unwrap(),
            factories: vec![FactoryConfig { address: factory }],
            chunk_size: 1000,
            poll_interval_secs: 1,
            use_latest: true,
            subgraph_url: PLACEHOLDER_SUBGRAPH_URL.parse().unwrap(),
            seed_block: None,
            fetch_concurrency: 8,
            prefetch_concurrency: 50,
        },
        api: ApiConfig {
            bind_address: SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::LOCALHOST, POOL_INDEXER_PORT)),
        },
        metrics: MetricsConfig {
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
    handle
}

/// Runs `body` with a freshly-started pool-indexer. The indexer is spawned
/// before the closure runs, then `abort`ed and `await`ed after — so the port
/// is fully released before this returns, and a follow-up call can re-bind it.
async fn with_pool_indexer<F, Fut, T>(factory: Address, body: F) -> T
where
    F: FnOnce() -> Fut,
    Fut: Future<Output = T>,
{
    with_pool_indexer_at(factory, 0, body).await
}

/// `with_pool_indexer` variant that lets the caller pick the metrics port —
/// only `driver_integration` needs this (it asserts on the metrics output).
async fn with_pool_indexer_at<F, Fut, T>(factory: Address, metrics_port: u16, body: F) -> T
where
    F: FnOnce() -> Fut,
    Fut: Future<Output = T>,
{
    let handle = spawn_pool_indexer(factory, metrics_port).await;
    let result = body().await;
    handle.abort();
    let _ = handle.await;
    result
}

/// Polls `/pools` until the indexer has reached `head` and surfaced at least
/// `min_pools` pools. `min_pools = 0` means "any state is fine, just check
/// block_number".
async fn wait_for_indexer(head: u64, min_pools: usize) {
    wait_for_condition(TIMEOUT, || async {
        let resp = reqwest::get(format!(
            "{POOL_INDEXER_HOST}/api/v1/mainnet/uniswap/v3/pools"
        ))
        .await
        .ok()?;
        let body: PoolsListResponse = resp.json().await.ok()?;
        Some(body.block_number >= head && body.pools.len() >= min_pools)
    })
    .await
    .expect("indexer did not reach target state");
}

/// Samples `(pool_count, sqrt_price_x96, tick, liquidity)` for a single pool.
async fn snapshot_pool_state(db: &PgPool, pool_addr: Address) -> (i64, String, i32, String) {
    let pool_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM uniswap_v3_pools")
        .fetch_one(db)
        .await
        .unwrap();
    let (sqrt_price, tick, liquidity): (String, i32, String) = sqlx::query_as(
        "SELECT sqrt_price_x96::TEXT, tick, liquidity::TEXT
         FROM uniswap_v3_pool_states
         WHERE pool_address = $1",
    )
    .bind(pool_addr.as_slice())
    .fetch_one(db)
    .await
    .unwrap();
    (pool_count, sqrt_price, tick, liquidity)
}

/// Create + initialise a single pool inside an already-deployed factory.
/// fee must be unique within the factory for token0/token1 ([1u8;20],[2u8;20]).
async fn create_pool(
    factory: &MockUniswapV3Factory::MockUniswapV3FactoryInstance<impl Provider>,
    fee: u32,
) -> Address {
    let provider = factory.provider();
    let token0 = Address::repeat_byte(1);
    let token1 = Address::repeat_byte(2);

    factory
        .createPool(token0, token1, U24::from(fee))
        .send()
        .await
        .unwrap()
        .watch()
        .await
        .unwrap();

    let block = provider.get_block_number().await.unwrap();
    let logs = provider
        .get_logs(
            &alloy::rpc::types::Filter::new()
                .from_block(block)
                .to_block(block)
                .event_signature(MockUniswapV3Factory::PoolCreated::SIGNATURE_HASH),
        )
        .await
        .unwrap();
    let pool_addr = MockUniswapV3Factory::PoolCreated::decode_log(&logs[0].inner)
        .unwrap()
        .data
        .pool;

    let pool = MockUniswapV3Pool::MockUniswapV3PoolInstance::new(pool_addr, provider);

    pool.initialize(U160::from(INITIAL_SQRT_PRICE))
        .send()
        .await
        .unwrap()
        .watch()
        .await
        .unwrap();

    pool.mockMint(
        token0,
        I24::try_from(-100i32).unwrap(),
        I24::try_from(100i32).unwrap(),
        1_000_000u128,
    )
    .send()
    .await
    .unwrap()
    .watch()
    .await
    .unwrap();

    pool_addr
}

/// Deploy mock V3 contracts and set up a pool with liquidity.
async fn deploy_univ3(
    web3: &Web3,
) -> (
    MockUniswapV3Factory::MockUniswapV3FactoryInstance<alloy::providers::DynProvider>,
    Address,
) {
    let provider = web3.provider.clone().erased();

    let factory = MockUniswapV3Factory::deploy(provider.clone())
        .await
        .unwrap();
    let pool_addr = create_pool(&factory, 500).await;

    (factory, pool_addr)
}

/// Parse the `pool_indexer_api_requests` Prometheus counter for a given
/// route from the indexer's /metrics endpoint.
async fn api_requests_counter(metrics_port: u16, route: &'static str) -> u64 {
    let body = reqwest::get(format!("http://127.0.0.1:{metrics_port}/metrics"))
        .await
        .unwrap()
        .text()
        .await
        .unwrap();
    let needle = format!("pool_indexer_api_requests{{route=\"{route}\"");
    for line in body.lines() {
        if line.starts_with('#') {
            continue;
        }
        if let Some(idx) = line.find(&needle) {
            // pool_indexer_api_requests{route="...",status="200"} 3
            let after = line[idx + needle.len()..].trim();
            if let Some(value) = after.split_whitespace().last() {
                return value.parse().unwrap_or(0);
            }
        }
    }
    0
}

#[tokio::test]
#[ignore]
async fn local_node_pool_indexer_driver_integration() {
    run_test(driver_integration).await;
}

/// Asserts (via the indexer's own request counters) that a driver pointed at
/// `pool-indexer-url` fetched pools AND their ticks. Ticks is the stronger
/// signal — only hit after `UniswapV3PoolFetcher::new` sees a non-empty set.
async fn driver_integration(web3: Web3) {
    const POOLS_ROUTE: &str = "/api/v1/{network}/uniswap/v3/pools";
    const POOLS_BY_IDS_ROUTE: &str = "/api/v1/{network}/uniswap/v3/pools/by-ids";
    const TICKS_ROUTE: &str = "/api/v1/{network}/uniswap/v3/pools/ticks";

    let db = PgPool::connect(LOCAL_DB_URL).await.unwrap();
    clear_pool_indexer_tables(&db).await;

    let mut onchain = OnchainComponents::deploy(web3.clone()).await;
    let [solver] = onchain.make_solvers(10u64.eth()).await;

    let (factory, pool_addr) = deploy_univ3(&web3).await;
    let factory_addr = *factory.address();
    let head = web3.provider.get_block_number().await.unwrap();
    seed_checkpoint(&db, factory_addr, 0).await;

    with_pool_indexer_at(factory_addr, POOL_INDEXER_METRICS_PORT, || async {
        // Without the min_pools=1 gate the driver could race against an empty
        // set and skip the ticks fetch this test asserts on.
        wait_for_indexer(head, 1).await;

        // Mock tokens have no real `decimals()`; backfill plausible values so
        // the driver's `pools_tokens_have_decimals` filter doesn't drop them.
        sqlx::query(
            "UPDATE uniswap_v3_pools SET token0_decimals = 18, token1_decimals = 6 WHERE address \
             = $1",
        )
        .bind(pool_addr.as_slice())
        .execute(&db)
        .await
        .unwrap();

        // Baseline AFTER warm-up polling so bumps below are driver-attributable.
        let baseline_pools = api_requests_counter(POOL_INDEXER_METRICS_PORT, POOLS_ROUTE).await;
        let baseline_pools_by_ids =
            api_requests_counter(POOL_INDEXER_METRICS_PORT, POOLS_BY_IDS_ROUTE).await;
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

        // Router address only used at settlement time; any 20-byte value works.
        let config_override = format!(
            r#"
[[liquidity.uniswap-v3]]
router = "0x000000000000000000000000000000000000dEaD"
indexer-config = {{ pool-indexer = {{ url = "{POOL_INDEXER_HOST}" }} }}
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
            let pools_by_ids =
                api_requests_counter(POOL_INDEXER_METRICS_PORT, POOLS_BY_IDS_ROUTE).await;
            let ticks = api_requests_counter(POOL_INDEXER_METRICS_PORT, TICKS_ROUTE).await;
            pools > baseline_pools && pools_by_ids > baseline_pools_by_ids && ticks > baseline_ticks
        })
        .await
        .expect("driver did not complete pool + tick fetch from pool-indexer within timeout");

        let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM uniswap_v3_pools")
            .fetch_one(&db)
            .await
            .unwrap();
        assert!(count > 0, "expected pools persisted to DB");

        driver_handle.abort();
    })
    .await;
}

#[tokio::test]
#[ignore]
async fn local_node_pool_indexer_checkpoint_resume() {
    run_test(checkpoint_resume).await;
}

/// Re-running the indexer over the same DB must merge into existing rows
/// (no duplicates) and leave per-pool state untouched. Asserts that pool
/// count, sqrt_price / tick / liquidity, and the checkpoint all survive a
/// stop+start.
async fn checkpoint_resume(web3: Web3) {
    let db = PgPool::connect(LOCAL_DB_URL).await.unwrap();
    clear_pool_indexer_tables(&db).await;

    let (factory, pool_addr) = deploy_univ3(&web3).await;
    let factory_addr = *factory.address();
    let head = web3.provider.get_block_number().await.unwrap();
    seed_checkpoint(&db, factory_addr, 0).await;

    let before = indexer_pass_snapshot(factory_addr, head, &db, pool_addr).await;
    let after = indexer_pass_snapshot(factory_addr, head, &db, pool_addr).await;
    assert_eq!(before, after, "indexer state changed across restart");

    let checkpoint: i64 = sqlx::query_scalar(
        "SELECT block_number FROM pool_indexer_checkpoints WHERE contract_address = $1",
    )
    .bind(factory_addr.as_slice())
    .fetch_one(&db)
    .await
    .unwrap();
    assert!(
        checkpoint as u64 >= head,
        "checkpoint did not advance to head"
    );
}

/// One full indexer lifecycle (start → wait for head → snapshot → stop).
/// Two calls in sequence over the same DB are how `checkpoint_resume`
/// proves the restart preserves state.
async fn indexer_pass_snapshot(
    factory_addr: Address,
    head: u64,
    db: &PgPool,
    pool_addr: Address,
) -> (i64, String, i32, String) {
    with_pool_indexer(factory_addr, || async {
        wait_for_indexer(head, 0).await;
        snapshot_pool_state(db, pool_addr).await
    })
    .await
}

#[tokio::test]
#[ignore]
async fn local_node_pool_indexer_api_errors() {
    run_test(api_errors).await;
}

/// Input-validation surface: an unparseable pool address must come back as
/// 400, a valid-but-unknown address must come back as 200 with empty ticks.
/// Lets callers distinguish "garbage input" from "no data yet".
async fn api_errors(web3: Web3) {
    let db = PgPool::connect(LOCAL_DB_URL).await.unwrap();
    clear_pool_indexer_tables(&db).await;

    let (factory, _pool) = deploy_univ3(&web3).await;
    let factory_addr = *factory.address();
    let head = web3.provider.get_block_number().await.unwrap();
    seed_checkpoint(&db, factory_addr, 0).await;

    with_pool_indexer(factory_addr, || async {
        wait_for_indexer(head, 0).await;
        invalid_address_returns_400().await;
        unknown_address_returns_empty_ticks().await;
    })
    .await;
}

async fn invalid_address_returns_400() {
    let status = reqwest::get(format!(
        "{POOL_INDEXER_HOST}/api/v1/mainnet/uniswap/v3/pools/not-an-address/ticks"
    ))
    .await
    .unwrap()
    .status();
    assert_eq!(u16::from(status), 400, "expected 400 for invalid address");
}

async fn unknown_address_returns_empty_ticks() {
    let unknown = Address::repeat_byte(0xAB);
    let resp: TicksResponse = reqwest::get(format!(
        "{POOL_INDEXER_HOST}/api/v1/mainnet/uniswap/v3/pools/{unknown:?}/ticks"
    ))
    .await
    .unwrap()
    .json()
    .await
    .unwrap();
    assert!(
        resp.ticks.is_empty(),
        "expected empty ticks for unknown pool"
    );
}

#[tokio::test]
#[ignore]
async fn local_node_pool_indexer_pagination() {
    run_test(pagination).await;
}

/// Cursor pagination: stepping through /pools with limit=1 must traverse
/// every pool exactly once. Three pools is the smallest set that exercises
/// a mid-stream cursor and the `next_cursor = null` terminator.
async fn pagination(web3: Web3) {
    let db = PgPool::connect(LOCAL_DB_URL).await.unwrap();
    clear_pool_indexer_tables(&db).await;

    let (factory, _pool1) = deploy_univ3(&web3).await;
    let _pool2 = create_pool(&factory, 3000).await;
    let _pool3 = create_pool(&factory, 10000).await;
    let factory_addr = *factory.address();
    let head = web3.provider.get_block_number().await.unwrap();
    seed_checkpoint(&db, factory_addr, 0).await;

    with_pool_indexer(factory_addr, || async {
        wait_for_indexer(head, 3).await;

        let mut all_ids: Vec<String> = Vec::new();
        let mut cursor: Option<String> = None;
        loop {
            let url = match &cursor {
                None => format!("{POOL_INDEXER_HOST}/api/v1/mainnet/uniswap/v3/pools?limit=1"),
                Some(c) => {
                    format!("{POOL_INDEXER_HOST}/api/v1/mainnet/uniswap/v3/pools?limit=1&after={c}")
                }
            };
            let resp: PoolsListResponse = reqwest::get(&url).await.unwrap().json().await.unwrap();
            if resp.pools.is_empty() {
                break;
            }
            for p in resp.pools {
                all_ids.push(p.id);
            }
            cursor = resp.next_cursor;
            if cursor.is_none() {
                break;
            }
        }

        let db_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM uniswap_v3_pools")
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
            "expected at least 3 pools to exercise pagination"
        );
        let unique: std::collections::HashSet<_> = all_ids.iter().collect();
        assert_eq!(
            unique.len(),
            all_ids.len(),
            "pagination returned duplicates"
        );
    })
    .await;
}
