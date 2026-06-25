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

const fn default_max_connections() -> NonZeroU32 {
    NonZeroU32::new(10).expect("non-zero literal")
}

const fn default_chunk_size() -> u64 {
    500
}

const fn default_poll_interval_secs() -> u64 {
    3
}

const fn default_fetch_concurrency() -> usize {
    8
}

const fn default_prefetch_concurrency() -> usize {
    50
}

const fn default_bind_address() -> SocketAddr {
    SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::UNSPECIFIED, 7777))
}

const fn default_metrics_address() -> SocketAddr {
    SocketAddr::V4(SocketAddrV4::new(
        Ipv4Addr::UNSPECIFIED,
        observe::metrics::DEFAULT_METRICS_PORT,
    ))
}

/// Network slug used in API routes (e.g. "mainnet", "arbitrum-one").
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
    /// Postgres connection URL. Accepts `%ENV_VAR` to pull from the
    /// environment.
    #[serde(deserialize_with = "configs::deserialize_env::deserialize_url_from_env")]
    pub url: Url,
    #[serde(default = "default_max_connections")]
    pub max_connections: NonZeroU32,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct NetworkConfig {
    pub name: NetworkName,
    pub chain_id: u64,
    #[serde(deserialize_with = "configs::deserialize_env::deserialize_url_from_env")]
    pub rpc_url: Url,
    /// Uniswap V3 factories to index. Exactly one is allowed in this release
    /// (see [`NetworkConfig::validate`]); multi-factory is a follow-up.
    pub factories: Vec<FactoryConfig>,
    /// Blocks per `eth_getLogs` chunk during catch-up.
    #[serde(default = "default_chunk_size")]
    pub chunk_size: u64,
    /// Interval for polling for new blocks during live indexing.
    #[serde(default = "default_poll_interval_secs")]
    pub poll_interval_secs: u64,
    /// Number of `eth_getLogs` chunks fetched in parallel during live indexing.
    #[serde(default = "default_fetch_concurrency")]
    pub fetch_concurrency: usize,
    /// `symbol()` / `decimals()` token-metadata RPC calls in flight during
    /// the backfill passes.
    #[serde(default = "default_prefetch_concurrency")]
    pub prefetch_concurrency: usize,
    /// Use `latest` instead of `finalized` as the indexing head. Set by tests
    /// against Anvil, which doesn't simulate finality.
    #[serde(skip)]
    pub use_latest: bool,
    /// Subgraph GraphQL endpoint for the initial seed.
    #[serde(deserialize_with = "configs::deserialize_env::deserialize_url_from_env")]
    pub subgraph_url: Url,
    /// Block to seed at. Defaults to the subgraph's current block.
    pub seed_block: Option<u64>,
}

impl NetworkConfig {
    pub fn poll_interval(&self) -> Duration {
        Duration::from_secs(self.poll_interval_secs)
    }

    pub fn indexer_config(&self, factory: Address) -> IndexerConfig {
        IndexerConfig {
            network: self.name.clone(),
            chain_id: self.chain_id,
            factory_address: factory,
            chunk_size: self.chunk_size,
            use_latest: self.use_latest,
            fetch_concurrency: self.fetch_concurrency,
            prefetch_concurrency: self.prefetch_concurrency,
        }
    }

    /// Post-parse sanity checks.
    fn validate(&self) -> Result<()> {
        anyhow::ensure!(
            self.factories.len() == 1,
            "network {}: exactly one factory per network is supported in this release, got {}",
            self.name,
            self.factories.len(),
        );
        Ok(())
    }
}

/// One factory address. The indexer runs a dedicated seed + live-indexing
/// loop per entry in [`NetworkConfig::factories`].
#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct FactoryConfig {
    pub address: Address,
}

/// Subset of [`NetworkConfig`] handed to [`UniswapV3Indexer`] at runtime.
#[derive(Debug, Clone)]
pub struct IndexerConfig {
    pub network: NetworkName,
    pub chain_id: u64,
    pub factory_address: Address,
    pub chunk_size: u64,
    pub use_latest: bool,
    pub fetch_concurrency: usize,
    pub prefetch_concurrency: usize,
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
pub struct MetricsConfig {
    #[serde(default = "default_metrics_address")]
    pub bind_address: SocketAddr,
}

impl Default for MetricsConfig {
    fn default() -> Self {
        Self {
            bind_address: default_metrics_address(),
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct Configuration {
    pub database: DatabaseConfig,
    pub network: NetworkConfig,
    #[serde(default)]
    pub api: ApiConfig,
    #[serde(default)]
    pub metrics: MetricsConfig,
}

impl Configuration {
    pub fn from_path(path: &Path) -> Result<Self> {
        let content = std::fs::read_to_string(path)
            .with_context(|| format!("reading config file {}", path.display()))?;
        let parsed: Self = toml::from_str(&content).context("parsing config file")?;
        parsed.network.validate()?;
        Ok(parsed)
    }
}
