//! End-to-end check that the driver consumes pool data from `pool-indexer`
//! when `pool-indexer-url` is configured. Uses inline mock V3 contracts
//! (bytecode embedded from PR #4349's solc 0.8.30 compilation — see
//! comments below) instead of the full contracts-generated pipeline.
//! Compared to the original `teamathon/indexer` harness this drops
//! ~3000 LOC of generated bindings; the bytecode constants below are the
//! only state that must be regenerated if the .sol sources ever change.
//!
//! Like the original harness, the test pre-seeds the pool-indexer
//! checkpoint so the subgraph_seeder bootstrap path is skipped — Anvil has
//! no subgraph to seed from. The test only exercises the live-indexing
//! and HTTP-serving paths, which is exactly the wiring the driver depends
//! on.

use {
    alloy::{
        primitives::{Address, aliases::U160},
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
    sqlx::PgPool,
    std::{
        net::{Ipv4Addr, SocketAddr, SocketAddrV4},
        num::NonZeroU32,
        sync::Mutex,
        time::Duration,
    },
};

// Minimal V3 Factory mock — emits the canonical `PoolCreated` event the
// indexer listens for. Bytecode below was compiled with solc 0.8.30,
// --optimize --optimize-runs 1000000, evm-version shanghai, from the
// source contracts/solidity/tests/MockUniswapV3Factory.sol on
// teamathon/indexer (PR #4349). The .sol source is reproduced here so a
// reviewer can verify the bytecode independently if needed.
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

// Minimal V3 Pool mock — emits `Initialize` and `Mint` events the indexer
// processes. Compiled identically to the factory above. Source on
// teamathon/indexer at contracts/solidity/tests/MockUniswapV3Pool.sol.
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

// Holds the JoinHandle of any currently-running pool-indexer so we can
// abort it even if a previous test panicked before stopping it.
static CURRENT_HANDLE: Mutex<Option<tokio::task::JoinHandle<()>>> = Mutex::new(None);

const POOL_INDEXER_PORT: u16 = 7778;
const POOL_INDEXER_HOST: &str = "http://127.0.0.1:7778";
const POOL_INDEXER_METRICS_PORT: u16 = 7779;
const LOCAL_DB_URL: &str = "postgresql://";

// sqrt(1) * 2^96 — valid starting price
const INITIAL_SQRT_PRICE: u128 = 79_228_162_514_264_337_593_543_950_336;

// Pre-seeded checkpoint short-circuits the subgraph_seeder bootstrap.
// Anvil has no V3 subgraph; this URL is never queried.
const PLACEHOLDER_SUBGRAPH_URL: &str = "http://127.0.0.1:1/never-queried";

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

async fn start_pool_indexer_at(factory: Address, metrics_port: u16) {
    if let Some(old) = CURRENT_HANDLE.lock().unwrap().take() {
        old.abort();
    }
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
            factories: vec![FactoryConfig { address: factory }],
            chunk_size: 1000,
            poll_interval_secs: 1,
            use_latest: true,
            subgraph_url: PLACEHOLDER_SUBGRAPH_URL.parse().unwrap(),
            subgraph_bearer_token: None,
            seed_block: None,
            fetch_concurrency: 8,
            prefetch_concurrency: 50,
        }],
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
    *CURRENT_HANDLE.lock().unwrap() = Some(handle);
}

fn stop_pool_indexer() {
    if let Some(h) = CURRENT_HANDLE.lock().unwrap().take() {
        h.abort();
    }
}

/// Create + initialise a single pool inside an already-deployed factory.
/// fee must be unique within the factory for token0/token1 ([1u8;20],[2u8;20]).
async fn create_pool(
    factory: &MockUniswapV3Factory::MockUniswapV3FactoryInstance<impl Provider>,
    fee: u32,
) -> Address {
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
            let after = line[idx + needle.len()..].trim();
            // line looks like:
            //   pool_indexer_api_requests{route="...",status="200"} 3
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

/// End-to-end: pool-indexer indexes a mock V3 factory, driver starts with
/// `pool-indexer-url` pointing at the service, and we assert (via the
/// indexer's own request counters) that the driver fetched both pools AND
/// their ticks. The ticks call is the stronger signal — it fires only
/// after `UniswapV3PoolFetcher::new` has a non-empty registered-pool set.
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

    start_pool_indexer_at(factory_addr, POOL_INDEXER_METRICS_PORT).await;

    // Wait until the indexer has both caught up to head AND surfaced the
    // seeded pool. Without the has_pool gate, the driver could race in
    // against an empty registered-pool set and silently skip the ticks
    // fetch this test wants to assert on.
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

    // Mock tokens (`[1u8;20]`, `[2u8;20]`) don't have a real `decimals()`
    // selector; the indexer stores NULL decimals and the driver-side
    // `pools_tokens_have_decimals` filter drops them, leaving the top-N
    // selection empty and skipping the bulk-by-ids/ticks fetch path this
    // test wants to assert on. Backfill plausible decimals so the driver
    // doesn't drop the pool.
    sqlx::query(
        "UPDATE uniswap_v3_pools SET token0_decimals = 18, token1_decimals = 6 WHERE chain_id = 1 \
         AND address = $1",
    )
    .bind(pool_addr.as_slice())
    .execute(&db)
    .await
    .unwrap();

    // Capture baselines after all test-side warm-up so the final
    // assertions prove the bumps came from the driver, not the polling
    // above.
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

    // The router address is only used at settlement time — any 20-byte
    // value is fine for a pool-fetch-only integration test.
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
        let pools_by_ids =
            api_requests_counter(POOL_INDEXER_METRICS_PORT, POOLS_BY_IDS_ROUTE).await;
        let ticks = api_requests_counter(POOL_INDEXER_METRICS_PORT, TICKS_ROUTE).await;
        pools > baseline_pools && pools_by_ids > baseline_pools_by_ids && ticks > baseline_ticks
    })
    .await
    .expect("driver did not complete pool + tick fetch from pool-indexer within timeout");

    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM uniswap_v3_pools WHERE chain_id = 1")
        .fetch_one(&db)
        .await
        .unwrap();
    assert!(count > 0, "expected pools persisted to DB");

    driver_handle.abort();
    stop_pool_indexer();
}
