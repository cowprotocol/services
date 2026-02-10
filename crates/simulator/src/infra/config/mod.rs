
use {
    alloy::eips::BlockNumberOrTag, serde::Deserialize, shared::{domain::eth, gas_price_estimation::configurable_alloy::{
        default_past_blocks,
        default_reward_percentile,
    }}, url::Url
};

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

/// Don't submit transactions with high revert risk (i.e. transactions
/// that interact with on-chain AMMs) to the public mempool.
/// This can be enabled to avoid MEV when private transaction
/// submission strategies are available. If private submission strategies
/// are not available, revert protection is always disabled.
#[derive(Debug, Clone, Copy)]
pub enum RevertProtection {
    Enabled,
    Disabled,
}

#[derive(Debug, Clone)]
pub struct MempoolConfig {
    pub min_priority_fee: eth::U256,
    pub gas_price_cap: eth::U256,
    pub target_confirm_time: std::time::Duration,
    pub retry_interval: std::time::Duration,
    /// Optional block number to use when fetching nonces. If None, uses the
    /// web3 lib's default behavior, which is `latest`.
    pub nonce_block_number: Option<BlockNumberOrTag>,
    pub url: Url,
    pub name: String,
    pub revert_protection: RevertProtection,
    pub max_additional_tip: eth::U256,
    pub additional_tip_percentage: f64,
}

#[derive(Debug)]
pub struct Config {
    pub gas_estimator: GasEstimatorType,
    pub mempools: Vec<MempoolConfig>,
    pub simulator: crate::Config,
    pub contracts: crate::infra::blockchain::contracts::Addresses,
    pub tx_gas_limit: eth::U256,
}