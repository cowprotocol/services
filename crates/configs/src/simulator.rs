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
    /// - enso (using EnsoConfig)
    pub kind: SimulatorKind,
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
pub enum SimulatorKind {
    #[default]
    Ethereum,
    Tenderly(TenderlyConfig),
    Enso(EnsoConfig),
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
    #[serde(default)]
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

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct EnsoConfig {
    /// The URL of the Transaction Simulator API.
    pub url: Url,

    /// The time between new blocks in the network.
    #[serde(default)]
    pub network_block_interval: Option<Duration>,
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
