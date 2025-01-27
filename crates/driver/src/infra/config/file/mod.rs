pub use load::load;
use {
    crate::{domain::eth, infra, util::serialize},
    reqwest::Url,
    serde::{Deserialize, Deserializer, Serialize},
    serde_with::serde_as,
    solver::solver::Arn,
    std::{collections::HashMap, time::Duration},
};

mod load;

#[serde_as]
#[derive(Debug, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
struct Config {
    /// Optionally specify the chain ID that that driver is configured for.
    /// Note that the actual chain ID is fetched from the configured Ethereum
    /// RPC endpoint, and the driver will exit if it does not match this
    /// value.
    chain_id: Option<u64>,

    /// Disable access list simulation, useful for environments that don't
    /// support this, such as less popular blockchains.
    #[serde(default)]
    disable_access_list_simulation: bool,

    /// Disable gas simulation and always use this fixed gas value instead. This
    /// can be useful for testing, but shouldn't be used in production since it
    /// will cause the driver to return invalid scores.
    #[serde_as(as = "Option<serialize::U256>")]
    disable_gas_simulation: Option<eth::U256>,

    /// Defines the gas estimator to use.
    #[serde(default)]
    gas_estimator: GasEstimatorType,

    /// Parameters related to settlement submission.
    #[serde(default)]
    submission: SubmissionConfig,

    /// Override smart contract addresses.
    #[serde(default)]
    contracts: ContractsConfig,

    /// Use Tenderly for transaction simulation.
    tenderly: Option<TenderlyConfig>,

    /// Use Enso for transaction simulation.
    enso: Option<EnsoConfig>,

    #[serde(rename = "solver")]
    solvers: Vec<SolverConfig>,

    #[serde(default)]
    liquidity: LiquidityConfig,

    /// Defines order prioritization strategies that will be applied in the
    /// specified order.
    #[serde(
        rename = "order-priority",
        default = "default_order_priority_strategies"
    )]
    order_priority_strategies: Vec<OrderPriorityStrategy>,

    /// Archive node URL used to index CoW AMM
    archive_node_url: Option<Url>,

    /// How long should the token quality computed by the simulation
    /// based logic be cached.
    #[serde(
        with = "humantime_serde",
        default = "default_simulation_bad_token_max_age"
    )]
    simulation_bad_token_max_age: Duration,

    /// Configuration for the app-data fetching.
    #[serde(default, flatten)]
    app_data_fetching: AppDataFetching,
}

#[serde_as]
#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
struct SubmissionConfig {
    /// The minimum priority fee in Gwei the solver is ensuring to pay in a
    /// settlement.
    #[serde(default)]
    #[serde_as(as = "serialize::U256")]
    min_priority_fee: eth::U256,

    /// The maximum gas price in Gwei the solver is willing to pay in a
    /// settlement.
    #[serde(default = "default_gas_price_cap")]
    #[serde_as(as = "serialize::U256")]
    gas_price_cap: eth::U256,

    /// The target confirmation time for settlement transactions used
    /// to estimate gas price.
    #[serde(with = "humantime_serde", default = "default_target_confirm_time")]
    target_confirm_time: Duration,

    /// Amount of time to wait before retrying to submit the tx to
    /// the ethereum network.
    #[serde(with = "humantime_serde", default = "default_retry_interval")]
    retry_interval: Duration,

    /// The mempools to submit settlement transactions to. Can be the public
    /// mempool of a node or the private MEVBlocker mempool.
    #[serde(rename = "mempool", default)]
    mempools: Vec<Mempool>,
}

#[serde_as]
#[derive(Debug, Deserialize)]
#[serde(tag = "mempool")]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
enum Mempool {
    #[serde(rename_all = "kebab-case")]
    Public {
        /// Maximum additional tip in Gwei that we are willing to pay
        /// above regular gas price estimation.
        #[serde(default = "default_max_additional_tip")]
        #[serde_as(as = "serialize::U256")]
        max_additional_tip: eth::U256,
        /// Additional tip in percentage of max_fee_per_gas we are willing to
        /// pay above regular gas price estimation. Expects a
        /// floating point value between 0 and 1.
        #[serde(default = "default_additional_tip_percentage")]
        additional_tip_percentage: f64,
    },
    #[serde(rename_all = "kebab-case")]
    MevBlocker {
        /// The MEVBlocker URL to use.
        url: Url,
        /// Maximum additional tip in Gwei that we are willing to give to
        /// MEVBlocker above regular gas price estimation.
        #[serde(default = "default_max_additional_tip")]
        #[serde_as(as = "serialize::U256")]
        max_additional_tip: eth::U256,
        /// Additional tip in percentage of max_fee_per_gas we are giving to
        /// MEVBlocker above regular gas price estimation. Expects a
        /// floating point value between 0 and 1.
        #[serde(default = "default_additional_tip_percentage")]
        additional_tip_percentage: f64,
        /// Configures whether the submission logic is allowed to assume the
        /// submission nodes implement soft cancellations. With soft
        /// cancellations a cancellation transaction doesn't have to get mined
        /// to have an effect. On arrival in the node all pending transactions
        /// with the same sender and nonce will get discarded immediately.
        #[serde(default = "default_soft_cancellations_flag")]
        use_soft_cancellations: bool,
    },
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
struct ManageNativeToken {
    /// If true wraps ETH address
    wrap_address: bool,
    /// If true inserts unwrap interactions
    insert_unwraps: bool,
}

impl Default for ManageNativeToken {
    fn default() -> Self {
        Self {
            wrap_address: true,
            insert_unwraps: true,
        }
    }
}

impl ManageNativeToken {
    pub fn to_domain(&self) -> infra::solver::ManageNativeToken {
        infra::solver::ManageNativeToken {
            wrap_address: self.wrap_address,
            insert_unwraps: self.insert_unwraps,
        }
    }
}

fn default_additional_tip_percentage() -> f64 {
    0.05
}

/// 1000 gwei
fn default_gas_price_cap() -> eth::U256 {
    eth::U256::from(1000) * eth::U256::exp10(9)
}

fn default_target_confirm_time() -> Duration {
    Duration::from_secs(30)
}

fn default_retry_interval() -> Duration {
    Duration::from_secs(2)
}

/// 3 gwei
fn default_max_additional_tip() -> eth::U256 {
    eth::U256::from(3) * eth::U256::exp10(9)
}

fn default_soft_cancellations_flag() -> bool {
    false
}

pub fn default_http_time_buffer() -> Duration {
    Duration::from_millis(500)
}

pub fn default_solving_share_of_deadline() -> f64 {
    0.8
}

#[serde_as]
#[derive(Debug, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
struct SolverConfig {
    /// The endpoint of this solver. `POST`ing an auction to this endpoint
    /// should prompt the solver to calculate and return a solution.
    endpoint: url::Url,

    /// The unique name for this solver. Used to disambiguate multiple solvers
    /// running behind a single driver.
    name: String,

    #[serde(flatten)]
    slippage: Slippage,

    /// Whether or not to skip fetching liquidity for this solver.
    #[serde(default)]
    skip_liquidity: bool,

    /// The account which should be used to sign settlements for this solver.
    account: Account,

    /// Timeout configuration for the solver.
    #[serde(default, flatten)]
    timeouts: Timeouts,

    #[serde(default)]
    request_headers: HashMap<String, String>,

    /// Determines whether the `solver` or the `driver` handles the fees
    #[serde(default)]
    fee_handler: FeeHandler,

    /// Use limit orders for quoting
    #[serde(default)]
    quote_using_limit_orders: bool,

    /// If enabled driver tries to merge multiple solutions for the same
    /// auction together.
    #[serde(default)]
    merge_solutions: bool,

    /// S3 configuration for storing the auctions in the form they are sent to
    /// the solver engine
    #[serde(default)]
    s3: Option<S3>,

    /// Whether the native token is wrapped or not when sent to the solvers
    #[serde(default)]
    manage_native_token: ManageNativeToken,

    /// Which `tx.origin` is required to make a quote simulation pass.
    #[serde(default)]
    quote_tx_origin: Option<eth::H160>,

    /// Maximum HTTP response size the driver will accept in bytes.
    #[serde(default = "default_response_size_limit_max_bytes")]
    response_size_limit_max_bytes: usize,

    /// Configuration for bad token detection.
    #[serde(default, flatten)]
    bad_token_detection: BadTokenDetectionConfig,

    /// The maximum number of `/settle` requests that can be queued up
    /// before the driver starts dropping new `/solve` requests.
    #[serde(default = "default_settle_queue_size")]
    settle_queue_size: usize,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub enum FeeHandler {
    #[default]
    Driver,
    Solver,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct S3 {
    /// Name of the AWS S3 bucket in which the auctions will be stored
    pub bucket: String,

    /// Prepended to the auction id to form the final instance filename on AWS
    /// S3 bucket. Something like "staging/mainnet/"
    pub prefix: String,
}

#[serde_as]
#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum Account {
    /// A private key is used to sign transactions. Expects a 32-byte hex
    /// encoded string.
    PrivateKey(eth::H256),
    /// AWS KMS is used to sign transactions. Expects the key identifier.
    Kms(#[serde_as(as = "serde_with::DisplayFromStr")] Arn),
    /// An address is used to identify the account for signing, relying on the
    /// connected node's account management features. This can also be used to
    /// start the driver in a dry-run mode.
    Address(eth::H160),
}

#[serde_as]
#[derive(Debug, Deserialize, Default)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
struct Timeouts {
    /// Absolute time allocated from the total auction deadline for
    /// request/response roundtrip between autopilot and driver.
    #[serde(with = "humantime_serde", default = "default_http_time_buffer")]
    http_time_buffer: Duration,

    /// Maximum time allocated for solver engines to return the solutions back
    /// to the driver, in percentage of total driver deadline (after network
    /// buffer). Remaining time is spent on encoding and postprocessing the
    /// returned solutions. Expected value [0, 1]
    #[serde(default = "default_solving_share_of_deadline")]
    solving_share_of_deadline: f64,
}

#[serde_as]
#[derive(Debug, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
struct Slippage {
    /// The relative slippage factor allowed by the solver.
    #[serde(rename = "relative-slippage")]
    #[serde_as(as = "serde_with::DisplayFromStr")]
    relative: bigdecimal::BigDecimal,

    /// The absolute slippage allowed by the solver.
    #[serde(rename = "absolute-slippage")]
    #[serde_as(as = "Option<serialize::U256>")]
    absolute: Option<eth::U256>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
struct ContractsConfig {
    /// Override the default address of the GPv2Settlement contract.
    gp_v2_settlement: Option<eth::H160>,

    /// Override the default address of the WETH contract.
    weth: Option<eth::H160>,

    /// List of all cow amm factories the driver should generate
    /// rebalancing orders for.
    #[serde(default)]
    cow_amms: Vec<CowAmmConfig>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct CowAmmConfig {
    /// Which contract to index for CoW AMM deployment events.
    pub factory: eth::H160,
    /// Which helper contract to use for interfacing with the indexed CoW AMMs.
    pub helper: eth::H160,
    /// At which block indexing should start on the factory.
    pub index_start: u64,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
struct TenderlyConfig {
    /// Optionally override the Tenderly API URL.
    url: Option<Url>,

    /// Authentication key for the Tenderly API.
    api_key: String,

    /// The Tenderly user associated with the API key.
    user: String,

    /// The Tenderly project associated with the API key.
    project: String,

    /// Save the transaction on Tenderly for later inspection, e.g. via the
    /// dashboard.
    save: bool,

    /// Save the transaction even in the case of failure.
    save_if_fails: bool,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
struct EnsoConfig {
    /// URL at which the trade simulator is hosted
    url: Url,
    /// How often the network produces a new block. If this is not set the
    /// system assumes an unpredictable network like proof-of-work.
    #[serde(default, with = "humantime_serde")]
    network_block_interval: Option<Duration>,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
struct LiquidityConfig {
    /// Additional tokens for which liquidity is always fetched, regardless of
    /// whether or not the token appears in the auction.
    #[serde(default)]
    base_tokens: Vec<eth::H160>,

    /// Liquidity provided by a Uniswap V2 compatible contract.
    #[serde(default)]
    uniswap_v2: Vec<UniswapV2Config>,

    /// Liquidity provided by a Swapr compatible contract.
    #[serde(default)]
    swapr: Vec<SwaprConfig>,

    /// Liquidity provided by a Uniswap V3 compatible contract.
    #[serde(default)]
    uniswap_v3: Vec<UniswapV3Config>,

    /// Liquidity provided by a Balancer V2 compatible contract.
    #[serde(default)]
    balancer_v2: Vec<BalancerV2Config>,

    /// Liquidity provided by 0x API.
    #[serde(default)]
    zeroex: Option<ZeroExConfig>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(untagged, deny_unknown_fields)]
enum UniswapV2Config {
    #[serde(rename_all = "kebab-case")]
    Preset { preset: UniswapV2Preset },

    #[serde(rename_all = "kebab-case")]
    Manual {
        /// The address of the Uniswap V2 compatible router contract.
        router: eth::H160,

        /// The digest of the pool initialization code.
        pool_code: eth::H256,

        /// How long liquidity should not be fetched for a token pair that
        /// didn't return useful liquidity before allowing to fetch it
        /// again.
        #[serde(with = "humantime_serde")]
        missing_pool_cache_time: Duration,
    },
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "kebab-case")]
enum UniswapV2Preset {
    UniswapV2,
    SushiSwap,
    Honeyswap,
    Baoswap,
    PancakeSwap,
    TestnetUniswapV2,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(untagged, deny_unknown_fields)]
enum SwaprConfig {
    #[serde(rename_all = "kebab-case")]
    Preset { preset: SwaprPreset },

    #[serde(rename_all = "kebab-case")]
    Manual {
        /// The address of the Swapr compatible router contract.
        router: eth::H160,

        /// The digest of the pool initialization code.
        pool_code: eth::H256,

        /// How long liquidity should not be fetched for a token pair that
        /// didn't return useful liquidity before allowing to fetch it
        /// again.
        #[serde(with = "humantime_serde")]
        missing_pool_cache_time: Duration,
    },
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
enum SwaprPreset {
    Swapr,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(untagged, deny_unknown_fields)]
enum UniswapV3Config {
    #[serde(rename_all = "kebab-case")]
    Preset {
        preset: UniswapV3Preset,

        /// How many pools to initialize during start up.
        #[serde(default = "uniswap_v3::default_max_pools_to_initialize")]
        max_pools_to_initialize: usize,

        graph_url: Url,
    },

    #[serde(rename_all = "kebab-case")]
    Manual {
        /// Addresses of Uniswap V3 compatible router contracts.
        router: eth::H160,

        /// How many pools to initialize during start up.
        #[serde(default = "uniswap_v3::default_max_pools_to_initialize")]
        max_pools_to_initialize: usize,

        /// The URL used to connect to uniswap v3 subgraph client.
        graph_url: Url,
    },
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
enum UniswapV3Preset {
    UniswapV3,
}

mod uniswap_v3 {
    pub fn default_max_pools_to_initialize() -> usize {
        100
    }
}

#[derive(Clone, Debug, Deserialize)]
#[serde(untagged, deny_unknown_fields)]
enum BalancerV2Config {
    #[serde(rename_all = "kebab-case")]
    Preset {
        preset: BalancerV2Preset,

        /// Deny listed Balancer V2 pools.
        #[serde(default)]
        pool_deny_list: Vec<eth::H256>,

        /// The URL used to connect to balancer v2 subgraph client.
        graph_url: Url,
    },

    #[serde(rename_all = "kebab-case")]
    Manual {
        /// Addresses of Balancer V2 compatible vault contract.
        vault: eth::H160,

        /// The weighted pool factory contract addresses.
        #[serde(default)]
        weighted: Vec<eth::H160>,

        /// The weighted pool factory v3+ contract addresses.
        #[serde(default)]
        weighted_v3plus: Vec<eth::H160>,

        /// The stable pool factory contract addresses.
        #[serde(default)]
        stable: Vec<eth::H160>,

        /// The liquidity bootstrapping pool factory contract addresses.
        ///
        /// These are weighted pools with dynamic weights for initial token
        /// offerings.
        #[serde(default)]
        liquidity_bootstrapping: Vec<eth::H160>,

        /// The composable stable pool factory contract addresses.
        #[serde(default)]
        composable_stable: Vec<eth::H160>,

        /// Deny listed Balancer V2 pools.
        #[serde(default)]
        pool_deny_list: Vec<eth::H256>,

        /// The URL used to connect to balancer v2 subgraph client.
        graph_url: Url,
    },
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
enum BalancerV2Preset {
    BalancerV2,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
struct ZeroExConfig {
    #[serde(default = "default_zeroex_base_url")]
    pub base_url: String,
    pub api_key: Option<String>,
    #[serde(with = "humantime_serde", default = "default_http_timeout")]
    pub http_timeout: Duration,
}

fn default_zeroex_base_url() -> String {
    "https://api.0x.org/".to_string()
}

fn default_http_timeout() -> Duration {
    Duration::from_secs(10)
}

fn default_response_size_limit_max_bytes() -> usize {
    30_000_000
}

#[derive(Clone, Debug, Deserialize, Default)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub enum GasEstimatorType {
    #[default]
    Native,
    Web3,
}

/// Defines various strategies to prioritize orders.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "kebab-case", tag = "strategy")]
pub enum OrderPriorityStrategy {
    /// Strategy to prioritize orders based on external price.
    /// This strategy uses the likelihood that an order will be fulfilled,
    /// based on token prices. A larger value means that the order is more
    /// likely to be fulfilled.
    ExternalPrice,
    /// Strategy to prioritize orders based on their creation timestamp. The
    /// most recently created orders are given the highest priority.
    #[serde(rename_all = "kebab-case")]
    CreationTimestamp {
        /// When specified, only orders created within this threshold will be
        /// taken into account for this specific strategy.
        #[serde(with = "humantime_serde", default = "default_max_order_age")]
        max_order_age: Option<Duration>,
    },
    /// Strategy to prioritize orders based on whether the current solver
    /// provided the winning quote for the order.
    #[serde(rename_all = "kebab-case")]
    OwnQuotes {
        /// When specified, only orders created within this threshold will be
        /// taken into account for this specific strategy.
        #[serde(with = "humantime_serde", default = "default_max_order_age")]
        max_order_age: Option<Duration>,
    },
}

/// The default prioritization process first considers
/// the order timestamp(2 minutes threshold by default), then checks if the
/// solver is working with its own quotes, and finally considers the likelihood
/// of order fulfillment based on external price data.
fn default_order_priority_strategies() -> Vec<OrderPriorityStrategy> {
    vec![
        OrderPriorityStrategy::OwnQuotes {
            max_order_age: default_max_order_age(),
        },
        OrderPriorityStrategy::CreationTimestamp {
            max_order_age: default_max_order_age(),
        },
        OrderPriorityStrategy::ExternalPrice,
    ]
}

fn default_max_order_age() -> Option<Duration> {
    Some(Duration::from_secs(300))
}

fn default_simulation_bad_token_max_age() -> Duration {
    Duration::from_secs(600)
}

#[serde_as]
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct BadTokenDetectionConfig {
    /// Which tokens are explicitly supported or unsupported by the solver.
    #[serde(default)]
    pub token_supported: HashMap<eth::H160, bool>,

    /// Whether the solver opted into detecting unsupported
    /// tokens with `trace_callMany` based simulation.
    #[serde(default, rename = "enable-simulation-bad-token-detection")]
    pub enable_simulation_strategy: bool,

    /// Whether the solver opted into detecting unsupported
    /// tokens with metrics-based detection.
    #[serde(default, rename = "enable-metrics-bad-token-detection")]
    pub enable_metrics_strategy: bool,

    /// The ratio of failures to attempts that qualifies a token as unsupported.
    #[serde(
        default = "default_metrics_bad_token_detector_failure_ratio",
        rename = "metrics-bad-token-detection-failure-ratio"
    )]
    pub metrics_strategy_failure_ratio: f64,

    /// The minimum number of attempts required before evaluating a token’s
    /// quality.
    #[serde(
        default = "default_metrics_bad_token_detector_required_measurements",
        rename = "metrics-bad-token-detection-required-measurements"
    )]
    pub metrics_strategy_required_measurements: u32,

    /// Controls whether the metrics based detection strategy should only log
    /// unsupported tokens or actually filter them out.
    #[serde(
        default = "default_metrics_bad_token_detector_log_only",
        rename = "metrics-bad-token-detection-log-only"
    )]
    pub metrics_strategy_log_only: bool,

    /// How long the metrics based bad token detection should flag a token as
    /// unsupported before it allows to solve for that token again.
    #[serde(
        default = "default_metrics_bad_token_detector_freeze_time",
        rename = "metrics-bad-token-detection-token-freeze-time",
        with = "humantime_serde"
    )]
    pub metrics_strategy_token_freeze_time: Duration,
}

impl Default for BadTokenDetectionConfig {
    fn default() -> Self {
        serde_json::from_str("{}").expect("MetricsBadTokenDetectorConfig uses default values")
    }
}

#[derive(Clone, Debug)]
pub enum AppDataFetching {
    /// App-data fetching is disabled
    Disabled,

    /// App-data fetching is enabled
    Enabled {
        /// The base URL of the orderbook to fetch app-data from
        orderbook_url: Url,

        /// The maximum number of app-data entries in the cache
        cache_size: u64,
    },
}

impl<'de> Deserialize<'de> for AppDataFetching {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[serde_as]
        #[derive(Deserialize)]
        #[serde(rename_all = "kebab-case", deny_unknown_fields)]
        struct Helper {
            #[serde(default)]
            app_data_fetching_enabled: bool,
            orderbook_url: Option<Url>,
            #[serde(default = "default_app_data_cache_size")]
            cache_size: u64,
        }

        let helper = Helper::deserialize(deserializer)?;
        match helper.app_data_fetching_enabled {
            false => Ok(AppDataFetching::Disabled),
            true => {
                let orderbook_url = helper
                    .orderbook_url
                    .ok_or_else(|| serde::de::Error::custom("Missing `orderbook-url` field"))?;
                Ok(AppDataFetching::Enabled {
                    orderbook_url,
                    cache_size: helper.cache_size,
                })
            }
        }
    }
}

fn default_metrics_bad_token_detector_failure_ratio() -> f64 {
    0.9
}

fn default_metrics_bad_token_detector_required_measurements() -> u32 {
    20
}

/// Keeps 2 requests in the queue plus 1 ongoing request making a total of 3
/// pending settlements, which is considered big enough to avoid potential price
/// moves or any other conflicts due to the extended settlement idle time.
fn default_settle_queue_size() -> usize {
    2
}

fn default_metrics_bad_token_detector_log_only() -> bool {
    true
}

fn default_metrics_bad_token_detector_freeze_time() -> Duration {
    Duration::from_secs(60 * 10)
}

/// According to statistics, the average size of the app-data is ~800 bytes.
/// With this default, the approximate size of the cache will be ~1.6 MB.
fn default_app_data_cache_size() -> u64 {
    2000
}
