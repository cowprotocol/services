use {std::net::SocketAddr, url::Url};

#[derive(Debug, clap::Parser)]
pub struct Args {
    /// The address to bind the driver to.
    #[clap(long, env)]
    pub bind_addr: SocketAddr,
    /// The node RPC API endpoint.
    #[clap(long, env)]
    pub ethrpc: Url,
    #[clap(flatten)]
    pub tenderly: Tenderly,
}

/// Arg types have custom `Display` impls instead of relying on `Debug` to avoid
/// accidentally printing secrets. Secret values are printed as "SECRET".
impl std::fmt::Display for Args {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "ethrpc: SECRET")?;
        write!(f, "bind_addr: {}", self.bind_addr)?;
        write!(f, "{}", self.tenderly)
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
        write!(
            f,
            "tenderly_api_key: {:?}",
            self.tenderly_api_key.as_ref().map(|_| "SECRET")
        )?;
        write!(f, "tenderly_user: {:?}", self.tenderly_user)?;
        write!(f, "tenderly_project: {:?}", self.tenderly_project)
    }
}

impl Tenderly {
    pub fn is_specified(&self) -> bool {
        if self.tenderly_url.is_none()
            && self.tenderly_api_key.is_none()
            && self.tenderly_user.is_none()
            && self.tenderly_project.is_none()
        {
            false
        } else if self.tenderly_api_key.is_some()
            && self.tenderly_user.is_none()
            && self.tenderly_project.is_none()
        {
            true
        } else {
            panic!("the tenderly args must all be specified together")
        }
    }
}
