use {
    crate::fee_factor::FeeFactor,
    alloy::primitives::Address,
    serde::{Deserialize, Deserializer, de::Unexpected},
    std::{collections::HashSet, str::FromStr, time::Duration},
    tracing::Level,
    url::Url,
};

fn default_node_url() -> Url {
    "http://localhost:8545".parse().unwrap()
}

fn default_gas_estimators() -> Vec<GasEstimatorType> {
    vec![GasEstimatorType::Web3]
}

const fn default_ethrpc_max_batch_size() -> usize {
    100
}

const fn default_ethrpc_max_concurrent_requests() -> usize {
    10
}

fn default_log_filter() -> String {
    String::from(
        "info,autopilot=debug,driver=debug,observe=info,orderbook=debug,solver=debug,shared=debug,\
         cow_amm=debug",
    )
}

const fn default_tracing_level() -> tracing::Level {
    tracing::Level::INFO
}

const fn default_tracing_exporter_timeout() -> Duration {
    Duration::from_secs(10)
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
#[cfg_attr(any(test, feature = "test-util"), derive(serde::Serialize))]
pub struct SharedConfig {
    #[serde(default)]
    pub ethrpc: EthRpcConfig,

    #[serde(default)]
    pub current_block: CurrentBlockConfig,

    #[serde(default)]
    pub logging: LoggingConfig,

    #[serde(default)]
    pub tracing: TracingConfig,

    #[serde(default = "default_node_url")]
    pub node_url: Url,

    #[serde(default)]
    pub simulation_node_url: Option<Url>,

    #[serde(default)]
    pub chain_id: Option<u64>,

    #[serde(default = "default_gas_estimators")]
    pub gas_estimators: Vec<GasEstimatorType>,

    #[serde(with = "humantime_serde", default)]
    pub network_block_interval: Option<Duration>,

    #[serde(default)]
    pub contracts: ContractAddresses,

    #[serde(default)]
    pub volume_fee_bucket_overrides: Vec<TokenBucketFeeOverride>,

    #[serde(default)]
    pub enable_sell_equals_buy_volume_fee: bool,
}

impl Default for SharedConfig {
    fn default() -> Self {
        Self {
            ethrpc: Default::default(),
            current_block: Default::default(),
            logging: Default::default(),
            tracing: Default::default(),
            node_url: default_node_url(),
            simulation_node_url: None,
            chain_id: None,
            gas_estimators: default_gas_estimators(),
            network_block_interval: None,
            contracts: Default::default(),
            volume_fee_bucket_overrides: Vec::new(),
            enable_sell_equals_buy_volume_fee: false,
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "kebab-case")]
#[cfg_attr(any(test, feature = "test-util"), derive(serde::Serialize))]
pub struct EthRpcConfig {
    #[serde(default = "default_ethrpc_max_batch_size")]
    pub max_batch_size: usize,

    #[serde(default = "default_ethrpc_max_concurrent_requests")]
    pub max_concurrent_requests: usize,

    #[serde(with = "humantime_serde", default)]
    pub batch_delay: Duration,
}

impl Default for EthRpcConfig {
    fn default() -> Self {
        Self {
            max_batch_size: default_ethrpc_max_batch_size(),
            max_concurrent_requests: default_ethrpc_max_concurrent_requests(),
            batch_delay: Duration::ZERO,
        }
    }
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "kebab-case")]
#[cfg_attr(any(test, feature = "test-util"), derive(serde::Serialize))]
pub struct CurrentBlockConfig {
    #[serde(with = "humantime_serde", default)]
    pub poll_interval: Option<Duration>,

    #[serde(default)]
    pub ws_url: Option<Url>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "kebab-case")]
#[cfg_attr(any(test, feature = "test-util"), derive(serde::Serialize))]
pub struct ContractAddresses {
    #[serde(default)]
    pub settlement: Option<Address>,

    #[serde(default)]
    pub balances: Option<Address>,

    #[serde(default)]
    pub signatures: Option<Address>,

    #[serde(default)]
    pub native_token: Option<Address>,

    #[serde(default)]
    pub hooks: Option<Address>,

    #[serde(default)]
    pub balancer_v2_vault: Option<Address>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "kebab-case")]
#[cfg_attr(any(test, feature = "test-util"), derive(serde::Serialize))]
pub struct LoggingConfig {
    #[serde(default = "default_log_filter")]
    pub filter: String,

    #[serde(default, deserialize_with = "deserialize_optional_level")]
    #[cfg_attr(
        any(test, feature = "test-util"),
        serde(
            skip_serializing_if = "Option::is_none",
            serialize_with = "serialize_optional_level"
        )
    )]
    pub stderr_threshold: Option<tracing::Level>,

    #[serde(default)]
    pub use_json: bool,
}

fn deserialize_optional_level<'de, D>(deserializer: D) -> Result<Option<tracing::Level>, D::Error>
where
    D: Deserializer<'de>,
{
    let Some(raw_level) = Option::<String>::deserialize(deserializer)? else {
        return Ok(None);
    };
    Ok(Some(tracing::Level::from_str(&raw_level).map_err(
        |_| {
            serde::de::Error::invalid_value(
                Unexpected::Str(&raw_level),
                // Since exp must be 'static, this string is copied from ParseLevelError::Display
                &"error parsing level: expected one of \"error\", \"warn\", \"info\", \"debug\", \
                  \"trace\", or a number 1-5",
            )
        },
    )?))
}

#[cfg(any(test, feature = "test-util"))]
fn serialize_optional_level<S>(
    level: &Option<tracing::Level>,
    serializer: S,
) -> Result<S::Ok, S::Error>
where
    S: serde::Serializer,
{
    match level {
        Some(level) => serializer.serialize_str(level.as_str()),
        None => serializer.serialize_none(),
    }
}

impl Default for LoggingConfig {
    fn default() -> Self {
        Self {
            filter: default_log_filter(),
            stderr_threshold: None,
            use_json: false,
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "kebab-case")]
#[cfg_attr(any(test, feature = "test-util"), derive(serde::Serialize))]
pub struct TracingConfig {
    #[serde(default)]
    pub collector_endpoint: Option<String>,

    #[serde(
        deserialize_with = "deserialize_level",
        default = "default_tracing_level"
    )]
    #[cfg_attr(
        any(test, feature = "test-util"),
        serde(serialize_with = "serialize_level")
    )]
    pub level: Level,

    #[serde(with = "humantime_serde", default = "default_tracing_exporter_timeout")]
    pub exporter_timeout: Duration,
}

impl Default for TracingConfig {
    fn default() -> Self {
        Self {
            collector_endpoint: None,
            level: default_tracing_level(),
            exporter_timeout: default_tracing_exporter_timeout(),
        }
    }
}

fn deserialize_level<'de, D>(deserializer: D) -> Result<tracing::Level, D::Error>
where
    D: Deserializer<'de>,
{
    let raw_level = String::deserialize(deserializer)?;
    tracing::Level::from_str(&raw_level).map_err(|_| {
        serde::de::Error::invalid_value(
            Unexpected::Str(&raw_level),
            // Since exp must be 'static, this string is copied from ParseLevelError::Display
            &"error parsing level: expected one of \"error\", \"warn\", \"info\", \"debug\", \
              \"trace\", or a number 1-5",
        )
    })
}

#[cfg(any(test, feature = "test-util"))]
fn serialize_level<S>(level: &tracing::Level, serializer: S) -> Result<S::Ok, S::Error>
where
    S: serde::Serializer,
{
    serializer.serialize_str(level.as_str())
}

#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "type")]
#[cfg_attr(any(test, feature = "test-util"), derive(serde::Serialize))]
pub enum GasEstimatorType {
    Web3,
    Driver { url: Url },
    Alloy,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "kebab-case")]
#[cfg_attr(any(test, feature = "test-util"), derive(serde::Serialize))]
pub struct TokenBucketFeeOverride {
    pub tokens: HashSet<Address>,
    pub factor: FeeFactor,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deserialize_shared_config_defaults() {
        let toml = "";
        let config: SharedConfig = toml::from_str(toml).unwrap();
        assert_eq!(config.node_url.as_str(), "http://localhost:8545/");
        assert!(config.chain_id.is_none());
        assert_eq!(config.gas_estimators.len(), 1);
        assert!(matches!(config.gas_estimators[0], GasEstimatorType::Web3));
        assert!(config.volume_fee_bucket_overrides.is_empty());
        assert!(!config.enable_sell_equals_buy_volume_fee);
    }

    #[test]
    fn deserialize_shared_config_full() {
        let toml = r#"
        node-url = "http://mainnet.example.com:8545"
        chain-id = 1
        enable-sell-equals-buy-volume-fee = true

        [contracts]
        settlement = "0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2"

        [ethrpc]
        max-batch-size = 200
        max-concurrent-requests = 20
        batch-delay = "1s"

        [current-block]
        poll-interval = "2s"
        ws-url = "ws://localhost:8546"

        [logging]
        filter = "debug"
        stderr-threshold = "warn"
        use-json = true

        [tracing]
        collector-endpoint = "http://jaeger:4317"
        level = "debug"
        exporter-timeout = "5s"

        [[gas-estimators]]
        type = "Web3"

        [[gas-estimators]]
        type = "Driver"
        url = "http://localhost:8080"

        [[volume-fee-bucket-overrides]]
        factor = 0.5
        tokens = [
            "0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2",
            "0x6B175474E89094C44Da98b954EedeAC495271d0F",
        ]
        "#;

        let config: SharedConfig = toml::from_str(toml).unwrap();
        assert_eq!(config.node_url.as_str(), "http://mainnet.example.com:8545/");
        assert_eq!(config.chain_id, Some(1));
        assert_eq!(config.ethrpc.max_batch_size, 200);
        assert_eq!(config.ethrpc.max_concurrent_requests, 20);
        assert_eq!(config.ethrpc.batch_delay, Duration::from_secs(1));
        assert_eq!(
            config.current_block.poll_interval,
            Some(Duration::from_secs(2))
        );
        assert!(config.current_block.ws_url.is_some());
        assert_eq!(config.logging.filter, "debug");
        assert_eq!(config.logging.stderr_threshold, Some(tracing::Level::WARN));
        assert!(config.logging.use_json);
        assert_eq!(
            config.tracing.collector_endpoint.as_deref(),
            Some("http://jaeger:4317")
        );
        assert_eq!(config.tracing.level, tracing::Level::DEBUG);
        assert_eq!(config.tracing.exporter_timeout, Duration::from_secs(5));
        assert_eq!(config.gas_estimators.len(), 2);
        assert!(config.contracts.settlement.is_some());
        assert!(config.enable_sell_equals_buy_volume_fee);
        assert_eq!(config.volume_fee_bucket_overrides.len(), 1);
        assert_eq!(config.volume_fee_bucket_overrides[0].tokens.len(), 2);
    }
}
