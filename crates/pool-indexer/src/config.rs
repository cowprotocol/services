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

const fn default_max_connections() -> NonZeroU32 {
    NonZeroU32::new(10).expect("non-zero literal")
}

const fn default_uniswap_v3_chunk_size() -> u64 {
    500
}

const fn default_balancer_v2_chunk_size() -> u64 {
    100_000
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
    /// Interval for polling for new blocks during live indexing.
    #[serde(default = "default_poll_interval_secs")]
    pub poll_interval_secs: u64,
    /// Number of `eth_getLogs` chunks fetched in parallel during live indexing.
    #[serde(default = "default_fetch_concurrency")]
    pub fetch_concurrency: usize,
    /// Token-metadata RPC calls (`decimals()`, plus `symbol()` for Uniswap V3)
    /// in flight during the backfill/enrich passes.
    #[serde(default = "default_prefetch_concurrency")]
    pub prefetch_concurrency: usize,
    /// Use `latest` instead of `finalized` as the indexing head. Set by tests
    /// against Anvil, which doesn't simulate finality.
    #[serde(skip)]
    pub use_latest: bool,
    /// Uniswap V3 pools to index. Set on its own or alongside `balancer_v2`.
    #[serde(default)]
    pub uniswap_v3: Option<UniswapV3Config>,
    /// Balancer V2 pools to index. Set on its own or alongside `uniswap_v3`.
    #[serde(default)]
    pub balancer_v2: Option<BalancerV2Config>,
}

impl NetworkConfig {
    pub fn poll_interval(&self) -> Duration {
        Duration::from_secs(self.poll_interval_secs)
    }

    pub fn indexer_config(&self, chunk_size: u64, factory: Address) -> IndexerConfig {
        IndexerConfig {
            network: self.name.clone(),
            chain_id: self.chain_id,
            factory_address: factory,
            chunk_size,
            use_latest: self.use_latest,
            fetch_concurrency: self.fetch_concurrency,
            prefetch_concurrency: self.prefetch_concurrency,
        }
    }

    /// Cross-field checks: index at least one protocol, and every factory
    /// address (across both) is unique — checkpoints are keyed by factory
    /// address in the shared `pool_indexer_checkpoints`, so a repeated address
    /// would drive two indexer loops onto one row.
    fn validate(&self) -> Result<()> {
        anyhow::ensure!(
            self.uniswap_v3.is_some() || self.balancer_v2.is_some(),
            "network {}: configure at least one of `uniswap-v3` or `balancer-v2`",
            self.name,
        );
        if let Some(balancer) = &self.balancer_v2 {
            anyhow::ensure!(
                balancer.factory_count() > 0,
                "network {}: balancer-v2 requires at least one factory",
                self.name,
            );
        }
        let uniswap_factories = self
            .uniswap_v3
            .iter()
            .flat_map(|u| &u.factories)
            .map(|f| f.address);
        let balancer_factories = self
            .balancer_v2
            .iter()
            .flat_map(|b| b.factories())
            .map(|f| f.address);
        let mut seen = HashSet::new();
        for factory in uniswap_factories.chain(balancer_factories) {
            anyhow::ensure!(
                seen.insert(factory),
                "network {}: factory {factory} configured more than once",
                self.name,
            );
        }
        Ok(())
    }
}

/// A pool factory and the block it was deployed at; the indexer cold-seeds by
/// replaying `PoolCreated` from `deploy_block`, then live-indexes. Shared by
/// the Uniswap V3 and Balancer V2 configs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct FactoryConfig {
    pub address: Address,
    /// Block the factory was deployed at; on-chain cold-seed scans from here.
    pub deploy_block: u64,
}

/// Uniswap V3 discovery config: the factories whose `PoolCreated` events the
/// indexer scans. Non-empty and unique, enforced at parse.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct UniswapV3Config {
    #[serde(deserialize_with = "serde_ext::deserialize_nonempty_unique_vec")]
    pub factories: Vec<FactoryConfig>,
    /// Blocks per `eth_getLogs` chunk. Small: the indexer fetches every pool's
    /// events chain-wide, so a wide range blows past RPC log-response caps.
    #[serde(default = "default_uniswap_v3_chunk_size")]
    pub chunk_size: u64,
}

/// Balancer V2 indexer config. Pools are created by per-type factories and
/// registered with a single Vault; the pool type is implied by which group a
/// factory is listed under.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct BalancerV2Config {
    pub vault: Address,
    /// Blocks per `eth_getLogs` chunk. Large: `PoolCreated` is factory-filtered
    /// (few logs), so the cold-seed scan stays a handful of calls.
    #[serde(default = "default_balancer_v2_chunk_size")]
    pub chunk_size: u64,
    #[serde(default)]
    pub weighted: Vec<FactoryConfig>,
    #[serde(default)]
    pub weighted_v3plus: Vec<FactoryConfig>,
    #[serde(default)]
    pub stable: Vec<FactoryConfig>,
    #[serde(default)]
    pub liquidity_bootstrapping: Vec<FactoryConfig>,
    #[serde(default)]
    pub composable_stable: Vec<FactoryConfig>,
}

impl BalancerV2Config {
    /// All configured factories, across every pool type.
    fn factories(&self) -> impl Iterator<Item = &FactoryConfig> {
        self.weighted
            .iter()
            .chain(&self.weighted_v3plus)
            .chain(&self.stable)
            .chain(&self.liquidity_bootstrapping)
            .chain(&self.composable_stable)
    }

    /// Total factories configured across all pool types.
    pub fn factory_count(&self) -> usize {
        self.factories().count()
    }
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
