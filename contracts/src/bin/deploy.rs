//! Script to deploy Gnosis Protocol v2 contracts to a local test network.
//! Additionally writes the deployed addresses to the `target` directory so that
//! they can be used by the build script.

use anyhow::{anyhow, bail, Context as _, Result};
use contracts::*;
use env_logger::Env;
use ethcontract::{Address, Http, Web3, U256};
use filetime::FileTime;
use std::{
    fs,
    path::Path,
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

    let network_id = web3.net().version().await.expect("get network ID failed");
    write_network_id(&network_id)?;
    log::info!("connected to test network {}", network_id);

    let accounts: Vec<Address> = web3.eth().accounts().await.expect("get accounts failed");
    let admin = accounts[0];

    macro_rules! deploy {
            ($contract:ident) => { deploy!($contract ()) };
            ($contract:ident ( $($param:expr),* $(,)? )) => {
                deploy!($contract ($($param),*) as stringify!($contract))
            };
            ($contract:ident ( $($param:expr),* $(,)? ) as $name:expr) => {{
                let name = $name;

                log::debug!("deploying {}...", name);
                let instance = $contract::builder(&web3 $(, $param)*)
                    .deploy()
                    .await
                    .with_context(|| format!("failed to deploy {}", name))?;

                log::debug!(
                    "writing deployment to {}",
                    paths::contract_address_file(name).display(),
                );
                write_contract_address(name, instance.address())
                    .with_context(|| format!("failed to write contract address for {}", name))?;

                log::info!("deployed {} to {:?}", name, instance.address());
                instance
            }};
        }

    log::info!("deploying WETH");
    let weth = deploy!(WETH9());

    log::info!("deploying Balancer V2");
    let balancer_authorizer = deploy!(BalancerV2Authorizer(admin));
    let balancer_vault = deploy!(BalancerV2Vault(
        balancer_authorizer.address(),
        weth.address(),
        U256::from(0),
        U256::from(0),
    ));

    log::info!("deploying Uniswap");
    let uniswap_factory = deploy!(UniswapV2Factory(accounts[0]));
    deploy!(UniswapV2Router02(uniswap_factory.address(), weth.address()));

    log::info!("deploying Gnosis Protocol V2");
    let gp_authentication = deploy!(GPv2AllowListAuthentication);
    gp_authentication
        .initialize_manager(admin)
        .send()
        .await
        .expect("failed to initialize manager");
    let settlement = deploy!(GPv2Settlement(
        gp_authentication.address(),
        balancer_vault.address(),
    ));

    log::info!("adding solver {:?}", admin);
    gp_authentication
        .add_solver(admin)
        .send()
        .await
        .expect("failed to allow list account 0");

    log::info!("authorizing Vault relayer");
    vault::grant_required_roles(
        balancer_authorizer,
        balancer_vault.address(),
        settlement
            .vault_relayer()
            .call()
            .await
            .expect("failed to retrieve Vault relayer contract address"),
    )
    .await
    .expect("failed to authorize Vault relayer");

    touch_build_script()
}

/// Writes the network ID to the workspace `target` directory.
fn write_network_id(network_id: &str) -> Result<()> {
    let path = paths::network_id_file();
    let dir = path
        .parent()
        .ok_or_else(|| anyhow!("network ID file does not have a parent directory"))?;

    fs::create_dir_all(dir)?;
    fs::write(path, network_id)?;

    Ok(())
}

/// Writes the deployed contract address to the workspace `target` directory.
fn write_contract_address(name: &str, address: Address) -> Result<()> {
    let path = paths::contract_address_file(name);
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
        tokio::time::sleep(NODE_READY_POLL_INTERVAL).await;
    }

    bail!(
        "Timed out waiting for node after {}s",
        NODE_READY_TIMEOUT.as_secs(),
    )
}
