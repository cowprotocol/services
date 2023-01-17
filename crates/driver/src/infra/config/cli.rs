use {crate::domain::eth, std::path::PathBuf, url::Url};

// TODO Move these different types into submodules in a follow-up

#[derive(Debug, clap::Parser)]
pub struct Args {
    /// The address to bind the driver to. "auto" to bind to 0.0.0.0 and any
    /// free port.
    #[clap(long, env, default_value = "0.0.0.0:11088")]
    pub bind_addr: String,

    /// The node RPC API endpoint.
    #[clap(long, env)]
    pub ethrpc: Url,

    /// Path to the solvers configuration file. This file should be in YAML
    /// format. For an example see
    /// https://github.com/cowprotocol/services/blob/main/crates/driver/example.solvers.toml.
    #[clap(long, env)]
    pub solvers_config: PathBuf,

    #[clap(flatten)]
    pub contract_addresses: ContractAddresses,

    #[clap(flatten)]
    pub tenderly: Tenderly,

    #[clap(flatten)]
    pub submission: Submission,

    /// Disable access list simulation, useful for environments that don't
    /// support this, such as less popular blockchains.
    #[clap(long, env)]
    pub disable_access_list_simulation: bool,

    /// The time to allocate to generating quotes, in milliseconds.
    #[clap(long, env, default_value = "5000")]
    pub quote_timeout_ms: u64,

    /// The mempools to use when submitting settlements.
    #[clap(
        long,
        env,
        default_value = "public",
        value_enum,
        use_value_delimiter = true
    )]
    pub mempools: Vec<Mempool>,

    /// The Flashbots API URL.
    #[clap(
        long,
        env,
        use_value_delimiter = true,
        default_value = "https://rpc.flashbots.net"
    )]
    pub flashbots_api_urls: Vec<Url>,

    /// The solver address used to sign transactions. Expects a 20-byte hex
    /// encoded string. This can't be specified along with --solver-private-key,
    /// exactly one of them must be specified.
    #[clap(long, env)]
    pub solver_address: Option<eth::H160>,

    /// The private key used to sign transactions. Expects a 32-byte hex encoded
    /// string. This can't be specified along with --solver-address, exactly one
    /// of them must be specified.
    #[clap(long, env)]
    pub solver_private_key: Option<String>,

    /// BlockNative API key. This is required for BlockNative gas price
    /// calculation.
    #[clap(long, env)]
    pub blocknative_api_key: Option<String>,
}

/// Arg types have custom `Display` impls instead of relying on `Debug` to avoid
/// accidentally printing secrets. Secret values are printed as "SECRET".
impl std::fmt::Display for Args {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "bind_addr: {}", self.bind_addr)?;
        writeln!(f, "ethrpc: SECRET")?;
        writeln!(f, "solvers_config: {:?}", self.solvers_config)?;
        writeln!(f, "solvers_config: {:?}", self.solvers_config)?;
        writeln!(f, "{}", self.contract_addresses)?;
        writeln!(f, "{}", self.tenderly)?;
        writeln!(
            f,
            "disable_access_list_simulation: {}",
            self.disable_access_list_simulation
        )?;
        writeln!(f, "quote_timeout_ms: {}", self.quote_timeout_ms)
    }
}

/// Tenderly API arguments.
#[derive(Debug, clap::Parser)]
pub struct Tenderly {
    /// The Tenderly API URL.
    #[clap(long, env)]
    pub tenderly_url: Option<Url>,

    /// Authentication key for the Tenderly API.
    #[clap(long, env)]
    pub tenderly_api_key: Option<String>,

    /// The Tenderly user associated with the API key.
    #[clap(long, env)]
    pub tenderly_user: Option<String>,

    /// The Tenderly project associated with the API key.
    #[clap(long, env)]
    pub tenderly_project: Option<String>,

    /// Save the transaction on Tenderly for later inspection, e.g. via the
    /// dashboard.
    #[clap(long, env)]
    pub tenderly_save: bool,

    /// Save the transaction as above, even in the case of failure.
    #[clap(long, env)]
    pub tenderly_save_if_fails: bool,
}

impl std::fmt::Display for Tenderly {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "tenderly_url: {:?}", self.tenderly_url)?;
        writeln!(
            f,
            "tenderly_api_key: {:?}",
            self.tenderly_api_key.as_ref().map(|_| "SECRET")
        )?;
        writeln!(f, "tenderly_user: {:?}", self.tenderly_user)?;
        writeln!(f, "tenderly_project: {:?}", self.tenderly_project)?;
        writeln!(f, "tenderly_save: {:?}", self.tenderly_save)?;
        writeln!(
            f,
            "tenderly_save_if_fails: {:?}",
            self.tenderly_save_if_fails
        )
    }
}

impl Tenderly {
    pub fn is_specified(&self) -> bool {
        if self.tenderly_url.is_none()
            && self.tenderly_api_key.is_none()
            && self.tenderly_user.is_none()
            && self.tenderly_project.is_none()
            && !self.tenderly_save
            && !self.tenderly_save_if_fails
        {
            false
        } else if self.tenderly_api_key.is_some()
            && self.tenderly_user.is_some()
            && self.tenderly_project.is_some()
        {
            true
        } else {
            panic!("the tenderly args must all be specified together")
        }
    }
}

/// Override smart contract addresses.
#[derive(Debug, clap::Parser)]
pub struct ContractAddresses {
    /// Override the default address of the GPv2Settlement contract.
    #[clap(long, env)]
    pub gp_v2_settlement: Option<eth::H160>,

    /// Override the default address of the WETH contract.
    #[clap(long, env)]
    pub weth: Option<eth::H160>,
}

impl std::fmt::Display for ContractAddresses {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "gp_v2_settlement: {:?}", self.gp_v2_settlement)?;
        writeln!(f, "weth: {:?}", self.weth)
    }
}

/// Parameters related to settlement submission.
#[derive(Debug, clap::Parser)]
pub struct Submission {
    /// How to calculate gas prices. Multiple approaches are used in sequence if
    /// the previous one fails.
    /// eth-gas-station: supports mainnet.
    /// gas-now: supports mainnet.
    /// gnosis-safe: supports mainnet and goerli.
    /// web3: supports every network.
    /// native: supports every network.
    #[clap(
        long,
        env,
        default_value = "web3",
        value_enum,
        use_value_delimiter = true
    )]
    pub submission_gas_price_calculation: Vec<GasPriceCalculation>,

    /// Additional tip in percentage of max_fee_per_gas we are willing to give
    /// to miners above regular gas price estimation. Expects a floating point
    /// value between 0 and 1.
    #[clap(long, env, default_value = "0.05")]
    pub submission_additional_tip_percentage: f64,

    /// The maximum gas price in Gwei the solver is willing to pay in a
    /// settlement.
    #[clap(long, env, default_value = "1500")]
    pub submission_gas_price_cap: f64,

    /// The target confirmation time for settlement transactions used
    /// to estimate gas price. Specified in seconds.
    #[clap(long, env, default_value = "30")]
    pub submission_target_confirm_time_secs: u64,

    /// Amount of time to wait before retrying to submit the tx to
    /// the ethereum network. Specified in seconds.
    #[clap(long, env, default_value = "2")]
    pub submission_retry_interval_secs: u64,

    /// Don't submit transactions with high revert risk (i.e. transactions that
    /// interact with on-chain AMMs) to the public mempool. This can be
    /// enabled to avoid MEV when private transaction submission strategies
    /// are available.
    #[clap(long, env)]
    pub submission_disable_high_risk_public_mempool_transactions: bool,

    /// The maximum time to spend trying to settle a transaction
    /// through the Ethereum network before going back to solving. Specified in
    /// seconds.
    #[clap(long, env, default_value = "120")]
    pub submission_max_confirm_time_secs: u64,

    /// Maximum additional tip in Gwei that we are willing to give to flashbots
    /// above regular gas price estimation.
    #[clap(long, env, default_value = "3")]
    pub submission_max_additional_flashbots_tip: f64,
}

#[derive(Debug, Clone, Copy, clap::ValueEnum)]
pub enum Mempool {
    Public,
    Flashbots,
}

#[derive(Debug, Copy, Clone, clap::ValueEnum)]
pub enum GasPriceCalculation {
    EthGasStation,
    GasNow,
    GnosisSafe,
    Web3,
    BlockNative,
    Native,
}
