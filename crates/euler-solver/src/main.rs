mod api;
mod config;
mod solver;

use {
    alloy::primitives::{Address, address}, chain::Chain, clap::Parser,
    std::net::SocketAddr,
};

fn parse_chain_id(s: &str) -> Result<Chain, String> {
    let chain_id: u64 = s.parse().map_err(|e| format!("invalid chain ID: {}", e))?;
    Chain::try_from(chain_id).map_err(|_| format!("unsupported chain ID: {}", chain_id))
}

#[derive(Parser, Debug)]
#[command(version, about = "Euler minimalist solver for CoW Protocol")]
struct Args {
    /// The socket address to bind to
    #[arg(long, env, default_value = "0.0.0.0:8080")]
    addr: SocketAddr,

    /// RPC node URL for interacting with the blockchain
    #[arg(long, env)]
    rpc_url: String,

    /// Chain ID (used to automatically determine contract addresses if not explicitly provided)
    #[arg(long, env, value_parser = parse_chain_id)]
    chain_id: Option<Chain>,

    /// WETH token address (overrides chain-id lookup)
    #[arg(long, env)]
    weth: Option<Address>,

    /// Uniswap V2 router address (defaults to mainnet address if not provided)
    #[arg(long, env)]
    uniswap_v2_router: Option<Address>,

    /// Settlement contract address (overrides chain-id lookup)
    #[arg(long, env)]
    settlement_contract_address: Option<Address>,

    /// The log filter
    #[arg(long, env, default_value = "info,euler_solver=debug")]
    log: String,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args = Args::parse();

    tracing::info!("Starting Euler solver");

    // Determine WETH address from args or chain_id
    let weth = match (args.chain_id, args.weth) {
        (Some(chain_id), None) => {
            let addr = config::get_weth_address(chain_id)?;
            tracing::info!("Using WETH address for chain {:?}: {:?}", chain_id, addr);
            addr
        }
        (_, Some(weth)) => {
            tracing::info!("Using WETH address from command line: {:?}", weth);
            weth
        }
        (None, None) => {
            anyhow::bail!("WETH address must be provided via --weth or --chain-id must be set");
        }
    };

    // Determine Uniswap V2 router address
    let uniswap_v2_router = args.uniswap_v2_router.unwrap_or_else(|| {
        let addr = address!("0x7a250d5630B4cF539739dF2C5dAcb4c659F2488D");
        tracing::info!("Using default Uniswap V2 router address: {:?}", addr);
        addr
    });

    // Determine settlement contract address from args or chain_id
    let settlement = match args.settlement_contract_address {
        Some(addr) => {
            tracing::info!("Using settlement address from command line: {:?}", addr);
            addr
        }
        None => {
            let chain_id = args.chain_id.ok_or_else(|| {
                anyhow::anyhow!(
                    "settlement contract address must be provided via --settlement-contract-address \
                     or --chain-id must be set"
                )
            })?;
            let addr = config::get_settlement_address(chain_id)?;
            tracing::info!(
                "Using settlement address for chain {:?}: {:?}",
                chain_id,
                addr
            );
            addr
        }
    };

    // Build config from command-line arguments
    let config = config::Config {
        rpc_url: args.rpc_url.clone(),
        weth,
        uniswap_v2_router,
        chain_id: args.chain_id,
    };
    tracing::info!("Config: {:?}", config);

    // Create RPC provider
    let web3 = ethrpc::Web3::new_from_url(&config.rpc_url);
    let provider = web3.provider;

    // Create solver
    let solver = solver::EulerSolver::new(
        provider,
        Address::from(settlement.0),
        Address::from(uniswap_v2_router.0),
    );

    // Create API server
    let api = api::Api {
        addr: args.addr,
        solver,
    };

    tracing::info!("Starting HTTP server on {}", args.addr);

    // Run server with graceful shutdown
    api.serve(None, shutdown_signal()).await?;

    Ok(())
}

#[cfg(unix)]
async fn shutdown_signal() {
    use tokio::signal::unix::{SignalKind, signal};

    let mut interrupt = signal(SignalKind::interrupt()).unwrap();
    let mut terminate = signal(SignalKind::terminate()).unwrap();

    tokio::select! {
        _ = interrupt.recv() => tracing::info!("Received SIGINT"),
        _ = terminate.recv() => tracing::info!("Received SIGTERM"),
    };
}

#[cfg(windows)]
async fn shutdown_signal() {
    tokio::signal::ctrl_c()
        .await
        .expect("failed to install CTRL+C signal handler");
}
