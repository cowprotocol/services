use {
    crate::gas_price_estimation::{default_past_blocks, default_reward_percentile},
    alloy::primitives::Address,
    serde::{Deserialize, Serialize},
    std::time::Duration,
    url::Url,
};

#[derive(Debug, Default, Clone, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct Config {
    /// Maximum batch size for Ethereum RPC requests. Use '0' to disable
    /// batching.
    #[serde(default = "default_ethrpc_max_batch_size")]
    pub ethrpc_max_batch_size: usize,

    /// Maximum number of concurrent requests to send to the node. Use '0' for
    /// no limit on concurrency.
    #[serde(default = "default_ethrpc_max_concurrent_requests")]
    pub ethrpc_max_concurrent_requests: usize,

    /// Buffering "nagle" delay to wait for additional requests before sending
    /// out an incomplete batch.
    // #[clap(long, env, value_parser = humantime::parse_duration, default_value = "0s")]
    #[serde(with = "humantime_serde", default = "default_ethrpc_batch_delay")]
    pub ethrpc_batch_delay: Duration,

    /// Kind of simulator that should be used. Can be either of
    /// - ethereum
    /// - tenderly (using TenderlyConfig)
    #[serde(default)]
    pub kind: SimulatorKind,

    /// Optional background websocket stream of eth_call-style state override
    /// sets, applied on top of latest state during settlement gas estimation.
    /// When absent, no stream task is spawned and behavior is unchanged.
    #[serde(default)]
    pub state_override_stream: Option<StateOverrideStream>,
}

/// Configuration for a background websocket stream delivering eth_call-style
/// state override sets (the standard Ethereum State Override Set wire format),
/// so live-quote venues (e.g. pAMMs) simulate against current in-memory state
/// rather than stale previous-block state.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct StateOverrideStream {
    /// Websocket URL pushing JSON frames of eth_call-style state overrides.
    pub ws_url: Url,

    /// Ignore the snapshot if the last frame is older than this.
    #[serde(with = "humantime_serde", default = "default_override_max_age")]
    pub max_age: Duration,

    /// Ignore the snapshot if its block number lags the current head by more
    /// than this many blocks.
    #[serde(default = "default_override_max_block_lag")]
    pub max_block_lag: u64,
}

const fn default_override_max_age() -> Duration {
    Duration::from_secs(3)
}

const fn default_override_max_block_lag() -> u64 {
    1
}

fn default_ethrpc_batch_delay() -> Duration {
    Duration::from_secs(0)
}
fn default_ethrpc_max_batch_size() -> usize {
    100
}
fn default_ethrpc_max_concurrent_requests() -> usize {
    10
}

#[derive(Debug, Default, Clone, Deserialize, Serialize)]
#[serde(tag = "type")]
pub enum SimulatorKind {
    #[default]
    Ethereum,
    Tenderly(Box<TenderlyConfig>),
}

/// Tenderly API arguments.
#[derive(Default, Clone, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct TenderlyConfig {
    /// The Tenderly user associated with the API key.
    #[serde(default)]
    pub user: String,

    /// The Tenderly project associated with the API key.
    #[serde(default)]
    pub project: String,

    /// Tenderly requires api key to work. Optional since Tenderly could be
    /// skipped in access lists estimators.
    /// Supports reading from an environment variable with the `%ENV_VAR`
    /// format.
    #[serde(
        default,
        deserialize_with = "crate::deserialize_env::deserialize_string_from_env"
    )]
    pub api_key: String,

    /// The URL of the Tenderly API.
    #[serde(default)]
    pub url: Option<Url>,

    /// The URL of the Tenderly dashboard
    #[serde(default)]
    pub dashboard: Option<Url>,

    /// Save the transaction on Tenderly for later inspection, e.g. via the
    /// dashboard.
    #[serde(default)]
    pub save: bool,

    /// Save the transaction even in the case of failure.
    #[serde(default)]
    pub save_if_fails: bool,
}

impl std::fmt::Debug for TenderlyConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TenderlyConfig")
            .field("user", &self.user)
            .field("project", &self.project)
            .field("api_key", &"<REDACTED>")
            .field("url", &self.url)
            .field("dashboard", &self.dashboard)
            .field("save", &self.save)
            .field("save_if_fails", &self.save_if_fails)
            .finish()
    }
}

#[cfg(any(test, feature = "test-util"))]
impl crate::test_util::TestDefault for TenderlyConfig {
    fn test_default() -> Self {
        Self {
            ..Default::default()
        }
    }
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields, tag = "estimator")]
pub enum GasEstimatorType {
    Web3,
    /// EIP-1559 gas estimator using alloy's algorithm.
    /// Optionally configure the fee history query parameters.
    #[serde(rename_all = "kebab-case")]
    Alloy {
        /// Number of blocks to look back for fee history (default: 10)
        #[serde(default = "default_past_blocks")]
        past_blocks: u64,
        /// Percentile of rewards to use for priority fee estimation (default:
        /// 20.0). This is what Metamask uses as medium priority:
        /// https://github.com/MetaMask/core/blob/0fd4b397e7237f104d1c81579a0c4321624d076b/packages/gas-fee-controller/src/fetchGasEstimatesViaEthFeeHistory/calculateGasFeeEstimatesForPriorityLevels.ts#L14-L45
        #[serde(default = "default_reward_percentile")]
        reward_percentile: f64,
    },
}

impl Default for GasEstimatorType {
    fn default() -> Self {
        Self::Alloy {
            past_blocks: default_past_blocks(),
            reward_percentile: default_reward_percentile(),
        }
    }
}

#[derive(Debug, Default, Clone, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct Addresses {
    pub settlement: Option<Address>,
    pub weth: Option<Address>,
}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deserialize_empty_config() {
        let config: Config = toml::from_str("").unwrap();
        assert!(matches!(config.kind, SimulatorKind::Ethereum));
    }

    #[test]
    fn deserialize_simulator_kind() {
        let toml = r#"
        [kind]
        type = "Tenderly"
        user = "test-user"
        project = "test-project"
        api-key = "test-api-key"
        "#;
        let config: Config = toml::from_str(toml).unwrap();

        match config.kind {
            SimulatorKind::Tenderly(tenderly) => {
                assert_eq!(tenderly.user, "test-user");
                assert_eq!(tenderly.project, "test-project");
                assert_eq!(tenderly.api_key, "test-api-key");
            }
            _ => panic!("Config should be of type Tenderly"),
        };
        let toml = r#"
        [kind]
        type = "Ethereum"
        "#;
        let config: Config = toml::from_str(toml).unwrap();
        assert!(matches!(config.kind, SimulatorKind::Ethereum))
    }

    #[derive(Deserialize)]
    struct TestConfigContainingSimulator {
        simulator: Config,
    }

    #[test]
    fn deserialize_full_config() {
        let toml = r#"
        [simulator.kind]
        type = "Tenderly"
        user = "test-user"
        api-key = "test-api-key"
        "#;
        let config: TestConfigContainingSimulator = toml::from_str(toml).unwrap();

        match config.simulator.kind {
            SimulatorKind::Tenderly(tenderly) => {
                assert_eq!(tenderly.user, "test-user");
                assert_eq!(tenderly.api_key, "test-api-key");
            }
            _ => panic!("Config should be of type Tenderly"),
        };
    }

    #[test]
    fn deserialize_state_override_stream_absent_by_default() {
        let config: Config = toml::from_str("").unwrap();
        assert!(config.state_override_stream.is_none());
    }

    #[test]
    fn deserialize_state_override_stream_with_defaults() {
        let toml = r#"
        [state-override-stream]
        ws-url = "wss://example.com/stream"
        "#;
        let config: Config = toml::from_str(toml).unwrap();
        let stream = config.state_override_stream.unwrap();
        assert_eq!(stream.ws_url.as_str(), "wss://example.com/stream");
        assert_eq!(stream.max_age, Duration::from_secs(3));
        assert_eq!(stream.max_block_lag, 1);
    }

    #[test]
    fn deserialize_state_override_stream_full() {
        let toml = r#"
        [state-override-stream]
        ws-url = "wss://example.com/stream"
        max-age = "5s"
        max-block-lag = 2
        "#;
        let config: Config = toml::from_str(toml).unwrap();
        let stream = config.state_override_stream.unwrap();
        assert_eq!(stream.max_age, Duration::from_secs(5));
        assert_eq!(stream.max_block_lag, 2);
    }

    #[test]
    fn deserialize_state_override_stream_rejects_unknown_fields() {
        let toml = r#"
        [state-override-stream]
        ws-url = "wss://example.com/stream"
        bogus = true
        "#;
        assert!(toml::from_str::<Config>(toml).is_err());
    }
}
