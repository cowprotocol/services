use {
    alloy::{
        network::TransactionBuilder,
        primitives::{Address, Bytes, aliases::U160},
        providers::Provider,
        rpc::types::TransactionRequest,
        sol_types::{SolCall, SolEvent},
    },
    e2e::setup::{TIMEOUT, run_test, wait_for_condition},
    ethrpc::{AlloyProvider, Web3},
    hex_literal::hex,
    pool_indexer::config::{ApiConfig, Configuration, DatabaseConfig, IndexerConfig},
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
const LOCAL_DB_URL: &str = "postgresql://";

// sqrt(1) * 2^96 — valid starting price
const INITIAL_SQRT_PRICE: u128 = 79_228_162_514_264_337_593_543_950_336;

// ABI types only (no bytecode — deployment uses raw transactions below).
alloy::sol! {
    contract MockUniswapV3Factory {
        event PoolCreated(
            address indexed token0,
            address indexed token1,
            uint24 indexed fee,
            int24 tickSpacing,
            address pool
        );
        function createPool(address tokenA, address tokenB, uint24 fee) external returns (address pool);
    }

    contract MockUniswapV3Pool {
        function initialize(uint160 sqrtPriceX96) external;
        function mockMint(address owner, int24 tickLower, int24 tickUpper, uint128 amount) external;
    }
}

// Factory bytecode compiled from Solidity via `forge build`. The factory
// embeds the pool constructor so only one bytecode blob is needed.
const FACTORY_BYTECODE: &[u8] = &hex!("6080604052348015600e575f5ffd5b50610b708061001c5f395ff3fe608060405234801561000f575f5ffd5b5060043610610029575f3560e01c8063a16712951461002d575b5f5ffd5b610047600480360381019061004291906101f2565b61005d565b6040516100549190610251565b60405180910390f35b5f5f5f8473ffffffffffffffffffffffffffffffffffffffff168673ffffffffffffffffffffffffffffffffffffffff161061009a57848661009d565b85855b915091508181856040516100b09061014f565b6100bc93929190610279565b604051809103905ff0801580156100d5573d5f5f3e3d5ffd5b5092508362ffffff168173ffffffffffffffffffffffffffffffffffffffff168373ffffffffffffffffffffffffffffffffffffffff167f783cca1c0412dd0d695e784568c96da2e9c22ff989357a2e8b1d9b2b4e6b7118600a8760405161013e9291906102fc565b60405180910390a450509392505050565b6108178061032483390190565b5f5ffd5b5f73ffffffffffffffffffffffffffffffffffffffff82169050919050565b5f61018982610160565b9050919050565b6101998161017f565b81146101a3575f5ffd5b50565b5f813590506101b481610190565b92915050565b5f62ffffff82169050919050565b6101d1816101ba565b81146101db575f5ffd5b50565b5f813590506101ec816101c8565b92915050565b5f5f5f606084860312156102095761020861015c565b5b5f610216868287016101a6565b9350506020610227868287016101a6565b9250506040610238868287016101de565b9150509250925092565b61024b8161017f565b82525050565b5f6020820190506102645f830184610242565b92915050565b610273816101ba565b82525050565b5f60608201905061028c5f830186610242565b6102996020830185610242565b6102a6604083018461026a565b949350505050565b5f819050919050565b5f8160020b9050919050565b5f819050919050565b5f6102e66102e16102dc846102ae565b6102c3565b6102b7565b9050919050565b6102f6816102cc565b82525050565b5f60408201905061030f5f8301856102ed565b61031c6020830184610242565b939250505056fe60e060405234801561000f575f5ffd5b5060405161081738038061081783398181016040528101906100319190610149565b8273ffffffffffffffffffffffffffffffffffffffff1660808173ffffffffffffffffffffffffffffffffffffffff16815250508173ffffffffffffffffffffffffffffffffffffffff1660a08173ffffffffffffffffffffffffffffffffffffffff16815250508062ffffff1660c08162ffffff1681525050505050610199565b5f5ffd5b5f73ffffffffffffffffffffffffffffffffffffffff82169050919050565b5f6100e0826100b7565b9050919050565b6100f0816100d6565b81146100fa575f5ffd5b50565b5f8151905061010b816100e7565b92915050565b5f62ffffff82169050919050565b61012881610111565b8114610132575f5ffd5b50565b5f815190506101438161011f565b92915050565b5f5f5f606084860312156101605761015f6100b3565b5b5f61016d868287016100fd565b935050602061017e868287016100fd565b925050604061018f86828701610135565b9150509250925092565b60805160a05160c0516106546101c35f395f61018101525f61015d01525f61011601526106545ff3fe608060405234801561000f575f5ffd5b5060043610610060575f3560e01c80630dfe1681146100645780631a68650214610082578063d21220a7146100a0578063ddca3f43146100be578063efe27fa3146100dc578063f637731d146100f8575b5f5ffd5b61006c610114565b60405161007991906102e1565b60405180910390f35b61008a610138565b6040516100979190610324565b60405180910390f35b6100a861015b565b6040516100b591906102e1565b60405180910390f35b6100c661017f565b6040516100d3919061035a565b60405180910390f35b6100f660048036038101906100f19190610401565b6101a3565b005b610112600480360381019061010d919061048f565b610266565b005b7f000000000000000000000000000000000000000000000000000000000000000081565b5f5f5f9054906101000a90046fffffffffffffffffffffffffffffffff16905090565b7f000000000000000000000000000000000000000000000000000000000000000081565b7f000000000000000000000000000000000000000000000000000000000000000081565b805f5f8282829054906101000a90046fffffffffffffffffffffffffffffffff166101ce91906104e7565b92506101000a8154816fffffffffffffffffffffffffffffffff02191690836fffffffffffffffffffffffffffffffff1602179055508160020b8360020b8573ffffffffffffffffffffffffffffffffffffffff167f7a53080ba414158be7ec69b987b5fb7d07dee101fe85488f0853ae16239d0bde33855f5f6040516102589493929190610575565b60405180910390a450505050565b7f98636036cb66a9c19a37435efc1e90142190214e8abeb821bdba3f2990dd4c95815f6040516102979291906105f7565b60405180910390a150565b5f73ffffffffffffffffffffffffffffffffffffffff82169050919050565b5f6102cb826102a2565b9050919050565b6102db816102c1565b82525050565b5f6020820190506102f45f8301846102d2565b92915050565b5f6fffffffffffffffffffffffffffffffff82169050919050565b61031e816102fa565b82525050565b5f6020820190506103375f830184610315565b92915050565b5f62ffffff82169050919050565b6103548161033d565b82525050565b5f60208201905061036d5f83018461034b565b92915050565b5f5ffd5b610380816102c1565b811461038a575f5ffd5b50565b5f8135905061039b81610377565b92915050565b5f8160020b9050919050565b6103b6816103a1565b81146103c0575f5ffd5b50565b5f813590506103d1816103ad565b92915050565b6103e0816102fa565b81146103ea575f5ffd5b50565b5f813590506103fb816103d7565b92915050565b5f5f5f5f6080858703121561041957610418610373565b5b5f6104268782880161038d565b9450506020610437878288016103c3565b9350506040610448878288016103c3565b9250506060610459878288016103ed565b91505092959194509250565b61046e816102a2565b8114610478575f5ffd5b50565b5f8135905061048981610465565b92915050565b5f602082840312156104a4576104a3610373565b5b5f6104b18482850161047b565b91505092915050565b7f4e487b71000000000000000000000000000000000000000000000000000000005f52601160045260245ffd5b5f6104f1826102fa565b91506104fc836102fa565b925082820190506fffffffffffffffffffffffffffffffff811115610524576105236104ba565b5b92915050565b5f819050919050565b5f819050919050565b5f819050919050565b5f61055f61055a6105558461052a565b61053c565b610533565b9050919050565b61056f81610545565b82525050565b5f6080820190506105885f8301876102d2565b6105956020830186610315565b6105a26040830185610566565b6105af6060830184610566565b95945050505050565b6105c1816102a2565b82525050565b5f6105e16105dc6105d78461052a565b61053c565b6103a1565b9050919050565b6105f1816105c7565b82525050565b5f60408201905061060a5f8301856105b8565b61061760208301846105e8565b939250505056fea264697066735822122010bef78e190e08820279eb6767095a2311ec4bd8c78aaa73b1854f67600215cf64736f6c634300081e0033a264697066735822122096722dfd3fb9e8ac23cc5b777ddf98ccc7720d61c61583a6e33fa33bcd92287164736f6c634300081e0033");

// ── helpers
// ───────────────────────────────────────────────────────────────────

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
    .bind(block as i64)
    .execute(db)
    .await
    .unwrap();
}

/// Start the pool-indexer. Aborts any previously-running instance first
/// (handles leftover from a prior test that panicked before calling
/// `stop_pool_indexer`).
async fn start_pool_indexer(factory: Address) {
    // Abort any handle left over from a previous test that panicked.
    if let Some(old) = CURRENT_HANDLE.lock().unwrap().take() {
        old.abort();
    }
    // Always wait a bit so the previous pool-indexer (if any) has time to
    // release port 7778 before we try to bind it again.
    tokio::time::sleep(Duration::from_millis(300)).await;

    let config = Configuration {
        database: DatabaseConfig {
            url: LOCAL_DB_URL.to_owned(),
            max_connections: NonZeroU32::new(5).unwrap(),
        },
        indexer: IndexerConfig {
            chain_id: 1,
            rpc_url: "http://127.0.0.1:8545".parse().unwrap(),
            factory_address: factory,
            chunk_size: 1000,
            poll_interval_secs: 1,
            use_latest: true,
        },
        api: ApiConfig {
            bind_address: SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::LOCALHOST, POOL_INDEXER_PORT)),
        },
    };
    let handle = tokio::task::spawn(pool_indexer::run(config, None, None));
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

async fn deploy_raw(provider: &AlloyProvider, bytecode: &[u8]) -> Address {
    let tx = TransactionRequest::default().with_deploy_code(Bytes::copy_from_slice(bytecode));
    provider
        .send_transaction(tx)
        .await
        .unwrap()
        .get_receipt()
        .await
        .unwrap()
        .contract_address
        .unwrap()
}

async fn send_call(provider: &AlloyProvider, to: Address, calldata: Vec<u8>) {
    let tx = TransactionRequest::default()
        .with_to(to)
        .with_input(Bytes::from(calldata));
    provider
        .send_transaction(tx)
        .await
        .unwrap()
        .get_receipt()
        .await
        .unwrap();
}

/// Create and initialise a single pool inside an already-deployed factory.
/// `fee` must be unique within the factory for token0/token1 ([1u8;20],
/// [2u8;20]).
async fn create_pool(provider: &AlloyProvider, factory_addr: Address, fee: u32) -> Address {
    let token0 = Address::from([1u8; 20]);
    let token1 = Address::from([2u8; 20]);

    send_call(
        provider,
        factory_addr,
        MockUniswapV3Factory::createPoolCall {
            tokenA: token0,
            tokenB: token1,
            fee: alloy::primitives::aliases::U24::from(fee),
        }
        .abi_encode(),
    )
    .await;

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

    send_call(
        provider,
        pool_addr,
        MockUniswapV3Pool::initializeCall {
            sqrtPriceX96: U160::from(INITIAL_SQRT_PRICE),
        }
        .abi_encode(),
    )
    .await;

    send_call(
        provider,
        pool_addr,
        MockUniswapV3Pool::mockMintCall {
            owner: token0,
            tickLower: alloy::primitives::aliases::I24::try_from(-100i32).unwrap(),
            tickUpper: alloy::primitives::aliases::I24::try_from(100i32).unwrap(),
            amount: 1_000_000u128,
        }
        .abi_encode(),
    )
    .await;

    pool_addr
}

/// Deploy mock V3 contracts and set up a pool with liquidity.
/// Returns `(factory_address, pool_address)`.
async fn deploy_v3(web3: &Web3) -> (Address, Address) {
    let provider = &web3.provider;

    let factory_addr = deploy_raw(provider, FACTORY_BYTECODE).await;

    // Two fixed addresses used as token0/token1 (sorted).
    let token0 = Address::from([1u8; 20]);
    let token1 = Address::from([2u8; 20]);
    debug_assert!(token0 < token1, "tokens must be sorted");

    // createPool(token0, token1, 500) → emits PoolCreated + deploys pool.
    send_call(
        provider,
        factory_addr,
        MockUniswapV3Factory::createPoolCall {
            tokenA: token0,
            tokenB: token1,
            fee: alloy::primitives::aliases::U24::from(500u32),
        }
        .abi_encode(),
    )
    .await;

    // Read pool address from the PoolCreated log.
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

    // initialize(sqrtPriceX96) → emits Initialize.
    send_call(
        provider,
        pool_addr,
        MockUniswapV3Pool::initializeCall {
            sqrtPriceX96: U160::from(INITIAL_SQRT_PRICE),
        }
        .abi_encode(),
    )
    .await;

    // mockMint(...) → emits Mint (indexer also calls pool.liquidity() after).
    send_call(
        provider,
        pool_addr,
        MockUniswapV3Pool::mockMintCall {
            owner: token0,
            tickLower: alloy::primitives::aliases::I24::try_from(-100i32).unwrap(),
            tickUpper: alloy::primitives::aliases::I24::try_from(100i32).unwrap(),
            amount: 1_000_000u128,
        }
        .abi_encode(),
    )
    .await;

    (factory_addr, pool_addr)
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[tokio::test]
#[ignore]
async fn local_node_pool_indexer_happy_path() {
    run_test(happy_path).await;
}

async fn happy_path(web3: Web3) {
    let db = PgPool::connect(LOCAL_DB_URL).await.unwrap();
    clear_pool_indexer_tables(&db).await;

    let (factory, pool_addr) = deploy_v3(&web3).await;
    let head = web3.provider.get_block_number().await.unwrap();

    seed_checkpoint(&db, factory, 0).await;
    start_pool_indexer(factory).await;

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

// ── Test 2: checkpoint resume
// ─────────────────────────────────────────────────

#[tokio::test]
#[ignore]
async fn local_node_pool_indexer_checkpoint_resume() {
    run_test(checkpoint_resume).await;
}

async fn checkpoint_resume(web3: Web3) {
    let db = PgPool::connect(LOCAL_DB_URL).await.unwrap();
    clear_pool_indexer_tables(&db).await;

    let (factory, pool_addr) = deploy_v3(&web3).await;
    let head = web3.provider.get_block_number().await.unwrap();
    seed_checkpoint(&db, factory, 0).await;

    start_pool_indexer(factory).await;
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

    start_pool_indexer(factory).await;
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
    .bind(factory.as_slice())
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

    let (factory, _pool_addr) = deploy_v3(&web3).await;
    let head = web3.provider.get_block_number().await.unwrap();

    seed_checkpoint(&db, factory, 0).await;
    start_pool_indexer(factory).await;

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
    let (factory, _pool1) = deploy_v3(&web3).await;
    create_pool(&web3.provider, factory, 3000).await;
    create_pool(&web3.provider, factory, 10_000).await;
    let head = web3.provider.get_block_number().await.unwrap();
    seed_checkpoint(&db, factory, 0).await;

    start_pool_indexer(factory).await;

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
        all_ids.len() as i64,
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
