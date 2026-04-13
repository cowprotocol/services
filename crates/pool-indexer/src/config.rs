use {
    alloy_primitives::Address,
    anyhow::{Context, Result},
    serde::Deserialize,
    std::{
        fmt,
        net::{Ipv4Addr, SocketAddr, SocketAddrV4},
        num::NonZeroU32,
        path::Path,
        time::Duration,
    },
    url::Url,
};

fn default_max_connections() -> NonZeroU32 {
    NonZeroU32::new(10).unwrap()
}

fn default_chunk_size() -> u64 {
    500
}

fn default_poll_interval_secs() -> u64 {
    3
}

fn default_bind_address() -> SocketAddr {
    SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::UNSPECIFIED, 7777))
}

/// Network identifier used in API routes (e.g. "mainnet", "arbitrum-one").
#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize)]
#[serde(transparent)]
pub struct NetworkName(String);

impl NetworkName {
    pub fn new(name: impl Into<String>) -> Self {
        Self(name.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for NetworkName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct DatabaseConfig {
    pub url: String,
    #[serde(default = "default_max_connections")]
    pub max_connections: NonZeroU32,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct NetworkConfig {
    pub name: NetworkName,
    pub chain_id: u64,
    pub rpc_url: Url,
    pub factory_address: Address,
    #[serde(default = "default_chunk_size")]
    pub chunk_size: u64,
    #[serde(default = "default_poll_interval_secs")]
    pub poll_interval_secs: u64,
    /// When `true`, use `latest` instead of `finalized` as the target block.
    /// Useful for test environments where finality is not simulated (e.g. local
    /// Anvil).
    #[serde(skip)]
    pub use_latest: bool,
    /// Subgraph GraphQL endpoint for seeding initial state. If absent, the
    /// indexer starts from genesis event indexing.
    pub subgraph_url: Option<String>,
    /// Block number to seed at. Defaults to the subgraph's current block when
    /// `subgraph_url` is set.
    pub seed_block: Option<u64>,
}

/// The subset of [`NetworkConfig`] that [`UniswapV3Indexer`] needs.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct IndexerConfig {
    pub chain_id: u64,
    pub rpc_url: Url,
    pub factory_address: Address,
    #[serde(default = "default_chunk_size")]
    pub chunk_size: u64,
    #[serde(default = "default_poll_interval_secs")]
    pub poll_interval_secs: u64,
    #[serde(skip)]
    pub use_latest: bool,
}

impl NetworkConfig {
    pub fn poll_interval(&self) -> Duration {
        Duration::from_secs(self.poll_interval_secs)
    }

    pub fn indexer_config(&self) -> IndexerConfig {
        IndexerConfig {
            chain_id: self.chain_id,
            rpc_url: self.rpc_url.clone(),
            factory_address: self.factory_address,
            chunk_size: self.chunk_size,
            poll_interval_secs: self.poll_interval_secs,
            use_latest: self.use_latest,
        }
    }
}

impl IndexerConfig {
    pub fn poll_interval(&self) -> Duration {
        Duration::from_secs(self.poll_interval_secs)
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct ApiConfig {
    #[serde(default = "default_bind_address")]
    pub bind_address: SocketAddr,
}

impl Default for ApiConfig {
    fn default() -> Self {
        Self {
            bind_address: default_bind_address(),
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct Configuration {
    pub database: DatabaseConfig,
    #[serde(rename = "network")]
    pub networks: Vec<NetworkConfig>,
    #[serde(default)]
    pub api: ApiConfig,
}

impl Configuration {
    pub fn from_path(path: &Path) -> Result<Self> {
        let content = std::fs::read_to_string(path)
            .with_context(|| format!("reading config file {}", path.display()))?;
        toml::from_str(&content).context("parsing config file")
    }
}
