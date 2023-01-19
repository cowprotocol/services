use {
    crate::{
        infra::{self, config::cli},
        tests::{self, hex_address, setup},
    },
    itertools::Itertools,
    std::{net::SocketAddr, path::PathBuf},
    tokio::{fs, sync::oneshot},
};

const SOLVERS_CONFIG_FILE: &str = "testing.solvers.toml";

pub const QUOTE_TIMEOUT_MS: u64 = 100;

/// HTTP client for talking to the driver API. Dropping the client shuts down
/// the running driver instance.
pub struct Client {
    addr: SocketAddr,
    client: reqwest::Client,
    handle: tokio::task::JoinHandle<()>,
}

impl Client {
    fn new(addr: SocketAddr, handle: tokio::task::JoinHandle<()>) -> Self {
        Self {
            addr,
            client: reqwest::Client::new(),
            handle,
        }
    }

    pub async fn solve(&self, solver: &str, req: serde_json::Value) -> serde_json::Value {
        let res = self
            .client
            .post(format!("http://{}/{solver}/solve", self.addr))
            .json(&req)
            .send()
            .await
            .unwrap();
        let status = res.status();
        let text = res.text().await.unwrap();
        tracing::debug!(?status, ?text, "got a response from /solve");
        serde_json::from_str(&text).unwrap()
    }

    pub async fn quote(&self, solver: &str, req: serde_json::Value) -> serde_json::Value {
        let res = self
            .client
            .post(format!("http://{}/{solver}/quote", self.addr))
            .json(&req)
            .send()
            .await
            .unwrap();
        let status = res.status();
        let text = res.text().await.unwrap();
        tracing::debug!(?status, ?text, "got a response from /quote");
        serde_json::from_str(&text).unwrap()
    }
}

impl Drop for Client {
    fn drop(&mut self) {
        self.handle.abort()
    }
}

#[derive(Debug)]
pub struct Config {
    pub now: infra::time::Now,
    pub contracts: cli::ContractAddresses,
    pub solvers: SolversConfig,
}

#[derive(Debug)]
pub enum SolversConfig {
    /// Create a new config file using [`SOLVERS_CONFIG_FILE`] for the given
    /// solvers.
    CreateConfigFile(Vec<setup::Solver>),
    /// Load an existing config file.
    LoadConfigFile(PathBuf),
}

/// Set up the driver.
pub async fn setup(config: Config) -> Client {
    let (addr_sender, addr_receiver) = oneshot::channel();
    let config_file = match config.solvers {
        SolversConfig::CreateConfigFile(solvers) => {
            create_solvers_config_file(&solvers).await;
            SOLVERS_CONFIG_FILE.into()
        }
        SolversConfig::LoadConfigFile(path) => path,
    };
    let solver_address = setup::blockchain::primary_account(&setup::blockchain::web3()).await;
    let mut args = vec![
        "/test/driver/path".to_owned(),
        "--bind-addr".to_owned(),
        "auto".to_owned(),
        "--solvers-config".to_owned(),
        config_file.to_str().unwrap().to_owned(),
        "--ethrpc".to_owned(),
        super::blockchain::WEB3_URL.to_owned(),
        "--quote-timeout-ms".to_owned(),
        QUOTE_TIMEOUT_MS.to_string(),
        "--solver-address".to_owned(),
        hex_address(solver_address),
    ];
    if let Some(settlement) = config.contracts.gp_v2_settlement {
        args.push("--gp-v2-settlement".to_owned());
        args.push(hex_address(settlement));
    }
    if let Some(weth) = config.contracts.weth {
        args.push("--weth".to_owned());
        args.push(hex_address(weth));
    }
    tests::boundary::initialize_tracing("debug,hyper=warn,driver::infra::solver=trace");
    let run = crate::run(args.into_iter(), config.now, Some(addr_sender));
    let handle = tokio::spawn(run);
    let driver_addr = addr_receiver.await.unwrap();
    Client::new(driver_addr, handle)
}

/// Create the config file for the solvers for the driver use.
async fn create_solvers_config_file(solvers: &[setup::Solver]) {
    let configs = solvers
        .iter()
        .map(|solver| {
            let setup::Solver {
                config:
                    setup::solver::Config {
                        absolute_slippage,
                        relative_slippage,
                        address,
                        name,
                        ..
                    },
                addr,
            } = solver;
            #[rustfmt::skip]
            let config = format!(
                "[[solvers]]\n\
                name = \"{name}\"\n\
                endpoint = \"http://{addr}\"\n\
                absolute-slippage = \"{absolute_slippage}\"\n\
                relative-slippage = \"{relative_slippage}\"\n\
                address = \"{address}\"\n"
            );
            config
        })
        .join("\n");
    fs::write(SOLVERS_CONFIG_FILE, configs).await.unwrap();
}
