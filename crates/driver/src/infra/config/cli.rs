use {
    crate::domain::eth,
    std::{path::PathBuf, str::FromStr},
    url::Url,
};

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
    /// format. For an example see TODO insert github link here.
    #[clap(long, env)]
    pub solvers_config: PathBuf,

    #[clap(flatten)]
    pub contract_addresses: ContractAddresses,

    #[clap(flatten)]
    pub tenderly: Tenderly,

    /// Disable access list simulation, useful for environments that don't
    /// support this, such as less popular blockchains.
    #[clap(long, env)]
    pub disable_access_list_simulation: bool,

    /// The time to allocate to generating quotes, in milliseconds.
    #[clap(long, env, default_value = "5000")]
    pub quote_timeout_ms: u64,
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
    pub gp_v2_settlement: Option<String>,

    /// Override the default address of the WETH contract.
    #[clap(long, env)]
    pub weth: Option<String>,
}

impl std::fmt::Display for ContractAddresses {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "gp_v2_settlement: {:?}", self.gp_v2_settlement)?;
        writeln!(f, "weth: {:?}", self.weth)
    }
}

pub fn hex_address(value: &str) -> eth::H160 {
    eth::H160::from_str(value).unwrap()
}
