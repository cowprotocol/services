use {
    crate::{
        infra::{self, config::cli},
        tests::{self, hex_address, setup},
    },
    itertools::Itertools,
    rand::Rng,
    std::{net::SocketAddr, path::PathBuf},
    tokio::{fs, sync::oneshot},
};

pub const QUOTE_TIMEOUT_MS: u64 = 100;

#[derive(Debug)]
struct ConfigPath(String);

impl ConfigPath {
    fn random() -> Self {
        let x: u32 = rand::thread_rng().gen();
        Self(format!("testing.{x}.toml"))
    }
}

/// HTTP client for talking to the driver API. Dropping the client shuts down
/// the running driver instance.
pub struct Client {
    addr: SocketAddr,
    client: reqwest::Client,
    handle: tokio::task::JoinHandle<()>,
    /// Delete this config file when the client is dropped.
    delete_config_file: Option<ConfigPath>,
}

impl Client {
    fn new(
        addr: SocketAddr,
        handle: tokio::task::JoinHandle<()>,
        delete_config_file: Option<ConfigPath>,
    ) -> Self {
        Self {
            addr,
            client: reqwest::Client::new(),
            handle,
            delete_config_file,
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
        assert_eq!(status, 200);
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
        assert_eq!(status, 200);
        serde_json::from_str(&text).unwrap()
    }

    pub async fn settle(&self, solver: &str, solution_id: &str) {
        let res = self
            .client
            .post(format!(
                "http://{}/{solver}/settle/{solution_id}",
                self.addr
            ))
            .send()
            .await
            .unwrap();
        let status = res.status();
        let text = res.text().await.unwrap();
        assert_eq!(status, 200);
        tracing::debug!(?status, ?text, "got a response from /settle");
    }
}

impl Drop for Client {
    fn drop(&mut self) {
        self.handle.abort();
        if let Some(config_file) = self.delete_config_file.as_ref() {
            std::fs::remove_file(&config_file.0).unwrap();
        }
    }
}

#[derive(Debug)]
pub struct Config<'a> {
    pub geth: &'a setup::blockchain::Geth,
    pub now: infra::time::Now,
    pub contracts: cli::ContractAddresses,
    pub file: ConfigFile,
}

#[derive(Debug)]
pub enum ConfigFile {
    /// Create a new config file using [`CONFIG_FILE`] for the given
    /// solvers.
    Create(Vec<setup::Solver>),
    /// Load an existing config file.
    Load(PathBuf),
}

/// Set up the driver.
pub async fn setup(config: Config<'_>) -> Client {
    let (addr_sender, addr_receiver) = oneshot::channel();
    let config_file = match &config.file {
        ConfigFile::Create(solvers) => {
            let path = ConfigPath::random();
            create_config_file(&path, solvers).await;
            path
        }
        ConfigFile::Load(path) => ConfigPath(path.to_str().unwrap().to_owned()),
    };
    let web3 = setup::blockchain::web3(&config.geth.url());
    let solver_address = setup::blockchain::primary_address(&web3).await;
    let mut args = vec![
        "/test/driver/path".to_owned(),
        "--bind-addr".to_owned(),
        "0.0.0.0:0".to_owned(),
        "--config".to_owned(),
        config_file.0.clone(),
        "--ethrpc".to_owned(),
        config.geth.url(),
        "--quote-timeout-ms".to_owned(),
        QUOTE_TIMEOUT_MS.to_string(),
        "--solver-address".to_owned(),
        hex_address(solver_address),
        "--submission-gas-price-cap".to_owned(),
        "1000000000000".to_owned(),
    ];
    if let Some(settlement) = config.contracts.gp_v2_settlement {
        args.push("--gp-v2-settlement".to_owned());
        args.push(hex_address(settlement));
    }
    if let Some(weth) = config.contracts.weth {
        args.push("--weth".to_owned());
        args.push(hex_address(weth));
    }
    tests::boundary::initialize_tracing("error,web3=warn,hyper=warn,driver::infra::solver=error");
    let run = crate::run(args.into_iter(), config.now, Some(addr_sender));
    let handle = tokio::spawn(run);
    let driver_addr = addr_receiver.await.unwrap();
    Client::new(
        driver_addr,
        handle,
        match config.file {
            ConfigFile::Create(_) => Some(config_file),
            ConfigFile::Load(_) => None,
        },
    )
}

/// Create the config file for the driver to use.
async fn create_config_file(path: &ConfigPath, solvers: &[setup::Solver]) {
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
                "[[solver]]\n\
                 name = \"{name}\"\n\
                 endpoint = \"http://{addr}\"\n\
                 absolute-slippage = \"{absolute_slippage}\"\n\
                 relative-slippage = \"{relative_slippage}\"\n\
                 address = \"{address}\"\n"
            );
            config
        })
        .join("\n");
    fs::write(&path.0, configs).await.unwrap();
}
