//! Script to deploy Gnosis Protocol v2 contracts to a local test network.
//! Additionally writes the deployed addresses to the `target` directory so that
//! they can be used by the build script.

use anyhow::{anyhow, bail, Context as _, Result};
use contracts::*;
use env_logger::Env;
use ethcontract::{Address, Http, Web3};
use filetime::FileTime;
use std::{
    fs,
    path::Path,
    thread,
    time::{Duration, Instant, SystemTime},
};

#[tokio::main]
async fn main() {
    env_logger::init_from_env(Env::default().default_filter_or("warn,deploy=info"));

    if let Err(err) = run().await {
        log::error!("Error deploying contracts: {:?}", err);
        std::process::exit(-1);
    }
}

async fn run() -> Result<()> {
    const NODE_URL: &str = "http://localhost:8545";

    let http = Http::new(NODE_URL)?;
    let web3 = Web3::new(http);

    log::info!("checking connection to local test node {}", NODE_URL);
    wait_for_node(&web3).await?;

    let accounts: Vec<Address> = web3.eth().accounts().await.expect("get accounts failed");

    macro_rules! deploy {
            ($contract:ident) => { deploy!($contract ()) };
            ($contract:ident ( $($param:expr),* $(,)? )) => {{
                const NAME: &str = stringify!($contract);

                log::debug!("deploying {}...", NAME);
                let instance = $contract::builder(&web3 $(, $param)*)
                    .gas(8_000_000_u32.into())
                    .deploy()
                    .await
                    .with_context(|| format!("failed to deploy {}", NAME))?;

                log::debug!(
                    "writing deployment to {}",
                    paths::contract_address_file(NAME).display(),
                );
                write_contract_address(stringify!($contract), instance.address())
                    .with_context(|| format!("failed to write contract address for {}", NAME))?;

                log::info!("deployed {} to {:?}", NAME, instance.address());
                instance
            }};
        }

    log::info!("deploying Uniswap contracts");
    let weth = deploy!(ERC20Mintable());
    let uniswap_factory = deploy!(UniswapV2Factory(accounts[0]));
    deploy!(UniswapV2Router02(uniswap_factory.address(), weth.address()));

    log::info!("deploying exchange contracts");
    let gp_authentication = deploy!(GPv2AllowListAuthentication(accounts[0]));
    gp_authentication
        .add_solver(accounts[0])
        .send()
        .await
        .expect("Failed to allow list account 0");
    deploy!(GPv2Settlement(gp_authentication.address()));

    touch_build_script()
}

/// Writes the deployed contract address to the workspace `target` directory.
fn write_contract_address(name: &str, address: Address) -> Result<()> {
    let path = paths::contract_address_file(name);
    let dir = path
        .parent()
        .ok_or_else(|| anyhow!("contract address path does not have a parent directory"))?;

    fs::create_dir_all(dir)?;
    fs::write(path, format!("{:?}", address))?;

    Ok(())
}

/// Touches the build in order to trigger a rebuild when a new deployment writes
/// contract address files.
///
/// See `build.rs` for more information.
fn touch_build_script() -> Result<()> {
    let timestamp = FileTime::from_system_time(SystemTime::now());
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("build.rs");

    filetime::set_file_times(path, timestamp, timestamp)?;

    Ok(())
}

/// Waits for the local development node to become available. Returns an error
/// if the node does not become available after a certain amount of time.
async fn wait_for_node(web3: &Web3<Http>) -> Result<()> {
    const NODE_READY_TIMEOUT: Duration = Duration::from_secs(30);
    const NODE_READY_POLL_INTERVAL: Duration = Duration::from_secs(1);

    let start = Instant::now();
    while start.elapsed() < NODE_READY_TIMEOUT {
        if web3.eth().accounts().await.is_ok() {
            return Ok(());
        }

        log::warn!(
            "node not responding, retrying in {}s",
            NODE_READY_POLL_INTERVAL.as_secs_f64(),
        );

        // NOTE: Usually a blocking call in a future is bad, but since we block
        // on this future right at the beginning and have no concurrent fibers,
        // it should be OK for this simple script.
        thread::sleep(NODE_READY_POLL_INTERVAL);
    }

    bail!(
        "Timed out waiting for node after {}s",
        NODE_READY_TIMEOUT.as_secs(),
    )
}
