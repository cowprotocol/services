use {
    alloy_primitives::Address,
    anyhow::{Context, Result},
    serde::Deserialize,
    std::{
        collections::HashSet,
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

fn default_fetch_concurrency() -> usize {
    8
}

fn default_prefetch_concurrency() -> usize {
    50
}

fn default_bind_address() -> SocketAddr {
    SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::UNSPECIFIED, 7777))
}

fn default_metrics_address() -> SocketAddr {
    SocketAddr::V4(SocketAddrV4::new(
        Ipv4Addr::UNSPECIFIED,
        observe::metrics::DEFAULT_METRICS_PORT,
    ))
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
    /// One or more Uniswap V3 factories to index. Each factory runs its own
    /// seed + live-indexing loop; pools from all factories share the per-chain
    /// namespace in the DB and API.
    pub factories: Vec<FactoryConfig>,
    #[serde(default = "default_chunk_size")]
    pub chunk_size: u64,
    #[serde(default = "default_poll_interval_secs")]
    pub poll_interval_secs: u64,
    #[serde(default = "default_fetch_concurrency")]
    pub fetch_concurrency: usize,
    #[serde(default = "default_prefetch_concurrency")]
    pub prefetch_concurrency: usize,
    /// When `true`, use `latest` instead of `finalized` as the target block.
    /// Useful for test environments where finality is not simulated (e.g. local
    /// Anvil).
    #[serde(skip)]
    pub use_latest: bool,
    /// Subgraph GraphQL endpoint for seeding initial state.
    #[serde(deserialize_with = "configs::deserialize_env::deserialize_url_from_env")]
    pub subgraph_url: Url,
    /// Block number to seed at. Defaults to the subgraph's current block.
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
}

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct FactoryConfig {
    pub address: Address,
}

/// The subset of [`NetworkConfig`] that [`UniswapV3Indexer`] needs at runtime.
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
    #[serde(rename = "network")]
    pub networks: Vec<NetworkConfig>,
    #[serde(default)]
    pub api: ApiConfig,
    #[serde(default)]
    pub metrics: MetricsConfig,
}

impl Configuration {
    pub fn from_path(path: &Path) -> Result<Self> {
        let content = std::fs::read_to_string(path)
            .with_context(|| format!("reading config file {}", path.display()))?;
        let config: Self = toml::from_str(&content).context("parsing config file")?;
        config.validate_networks()?;
        Ok(config)
    }

    /// Cross-network sanity checks that don't fit serde's per-field validation:
    /// uniqueness of names / chain IDs and the exactly-one-factory invariant.
    fn validate_networks(&self) -> Result<()> {
        anyhow::ensure!(
            !self.networks.is_empty(),
            "at least one [[network]] must be configured",
        );
        let mut names = HashSet::new();
        let mut chain_ids = HashSet::new();
        for n in &self.networks {
            anyhow::ensure!(
                names.insert(n.name.as_str()),
                "duplicate network name: {}",
                n.name,
            );
            anyhow::ensure!(
                chain_ids.insert(n.chain_id),
                "duplicate chain_id: {}",
                n.chain_id,
            );
            anyhow::ensure!(
                n.factories.len() == 1,
                "network {}: exactly one factory per network is supported in this release, got {}",
                n.name,
                n.factories.len(),
            );
        }
        Ok(())
    }
}
