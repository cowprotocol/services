use primitive_types::{H160, H256};
use reqwest::Url;
use shared::{
    arguments::{display_list, display_option, duration_from_seconds},
    gas_price_estimation::GasEstimatorType,
    sources::{balancer_v2::BalancerFactoryKind, BaselineSource},
};
use solver::{
    arguments::TransactionStrategyArg, settlement_access_list::AccessListEstimatorType,
    solver::ExternalSolverArg,
};
use std::{net::SocketAddr, num::NonZeroU64, time::Duration};
use tracing::level_filters::LevelFilter;

#[derive(clap::Parser)]
pub struct Arguments {
    #[clap(long, env, default_value = "0.0.0.0:8080")]
    pub bind_address: SocketAddr,

    #[clap(
        long,
        env,
        default_value = "warn,driver=debug,shared=debug,shared::transport::http=info"
    )]
    pub log_filter: String,

    #[clap(long, env, default_value = "error")]
    pub log_stderr_threshold: LevelFilter,

    /// List of solvers in the form of `name|url|account`.
    #[clap(long, env, use_value_delimiter = true)]
    pub solvers: Vec<ExternalSolverArg>,

    /// The Ethereum node URL to connect to.
    #[clap(long, env, default_value = "http://localhost:8545")]
    pub node_url: Url,

    /// Timeout in seconds for all http requests.
    #[clap(
        long,
        default_value = "10",
        parse(try_from_str = duration_from_seconds),
    )]
    pub http_timeout: Duration,

    /// If solvers should use internal buffers to improve solution quality.
    #[clap(long, env)]
    pub use_internal_buffers: bool,

    /// The RPC endpoints to use for submitting transaction to a custom set of nodes.
    #[clap(long, env, use_value_delimiter = true)]
    pub transaction_submission_nodes: Vec<Url>,

    /// Don't submit high revert risk (i.e. transactions that interact with on-chain
    /// AMMs) to the public mempool. This can be enabled to avoid MEV when private
    /// transaction submission strategies are available.
    #[clap(long, env)]
    pub disable_high_risk_public_mempool_transactions: bool,

    /// Fee scaling factor for objective value. This controls the constant
    /// factor by which order fees are multiplied with. Setting this to a value
    /// greater than 1.0 makes settlements with negative objective values less
    /// likely, promoting more aggressive merging of single order settlements.
    #[clap(long, env, default_value = "1", parse(try_from_str = shared::arguments::parse_unbounded_factor))]
    pub fee_objective_scaling_factor: f64,

    /// How to to submit settlement transactions.
    /// Expected to contain either:
    /// 1. One value equal to TransactionStrategyArg::DryRun or
    /// 2. One or more values equal to any combination of enum variants except TransactionStrategyArg::DryRun
    #[clap(
        long,
        env,
        default_value = "PublicMempool",
        arg_enum,
        ignore_case = true,
        use_value_delimiter = true
    )]
    pub transaction_strategy: Vec<TransactionStrategyArg>,

    /// The API endpoint of the Eden network for transaction submission.
    #[clap(long, env, default_value = "https://api.edennetwork.io/v1/rpc")]
    pub eden_api_url: Url,

    /// Maximum additional tip in gwei that we are willing to give to eden above regular gas price estimation
    #[clap(
        long,
        env,
        default_value = "3",
        parse(try_from_str = shared::arguments::wei_from_gwei)
    )]
    pub max_additional_eden_tip: f64,

    /// Additional tip in percentage of max_fee_per_gas we are willing to give to miners above regular gas price estimation
    #[clap(
        long,
        env,
        default_value = "0.05",
        parse(try_from_str = shared::arguments::parse_percentage_factor)
    )]
    pub additional_tip_percentage: f64,

    /// The API endpoint of the Flashbots network for transaction submission.
    /// Multiple values could be defined for different Flashbots endpoints (Flashbots Protect and Flashbots fast).
    #[clap(
        long,
        env,
        use_value_delimiter = true,
        default_value = "https://rpc.flashbots.net"
    )]
    pub flashbots_api_url: Vec<Url>,

    /// Maximum additional tip in gwei that we are willing to give to flashbots above regular gas price estimation
    #[clap(
        long,
        env,
        default_value = "3",
        parse(try_from_str = shared::arguments::wei_from_gwei)
    )]
    pub max_additional_flashbot_tip: f64,

    /// Which access list estimators to use. Multiple estimators are used in sequence if a previous one
    /// fails. Individual estimators might support different networks.
    /// `Tenderly`: supports every network.
    /// `Web3`: supports every network.
    #[clap(long, env, arg_enum, ignore_case = true, use_value_delimiter = true)]
    pub access_list_estimators: Vec<AccessListEstimatorType>,

    /// The URL for tenderly transaction simulation.
    #[clap(long, env)]
    pub tenderly_url: Option<Url>,

    /// Tenderly requires api key to work. Optional since Tenderly could be skipped in access lists estimators.
    #[clap(long, env)]
    pub tenderly_api_key: Option<String>,

    /// Gas limit for simulations. This parameter is important to set correctly, such that
    /// there are no simulation errors due to: err: insufficient funds for gas * price + value,
    /// but at the same time we don't restrict solutions sizes too much
    #[clap(long, env, default_value = "15000000")]
    pub simulation_gas_limit: u128,

    /// The target confirmation time in seconds for settlement transactions used to estimate gas price.
    #[clap(
        long,
        env,
        default_value = "30",
        parse(try_from_str = shared::arguments::duration_from_seconds),
    )]
    pub target_confirm_time: Duration,

    /// The maximum time in seconds we spend trying to settle a transaction through the ethereum
    /// network before going to back to solving.
    #[clap(
        long,
        default_value = "120",
        parse(try_from_str = shared::arguments::duration_from_seconds),
    )]
    pub max_submission_seconds: Duration,

    /// Amount of time to wait before retrying to submit the tx to the ethereum network
    #[clap(
        long,
        default_value = "2",
        parse(try_from_str = shared::arguments::duration_from_seconds),
    )]
    pub submission_retry_interval_seconds: Duration,

    /// The maximum gas price in Gwei the solver is willing to pay in a settlement.
    #[clap(
        long,
        env,
        default_value = "1500",
        parse(try_from_str = shared::arguments::wei_from_gwei)
    )]
    pub gas_price_cap: f64,

    /// Which gas estimators to use. Multiple estimators are used in sequence if a previous one
    /// fails. Individual estimators support different networks.
    /// `EthGasStation`: supports mainnet.
    /// `GasNow`: supports mainnet.
    /// `GnosisSafe`: supports mainnet, rinkeby and goerli.
    /// `Web3`: supports every network.
    /// `Native`: supports every network.
    #[clap(
        long,
        env,
        default_value = "Web3",
        arg_enum,
        ignore_case = true,
        use_value_delimiter = true
    )]
    pub gas_estimators: Vec<GasEstimatorType>,

    /// BlockNative requires api key to work. Optional since BlockNative could be skipped in gas estimators.
    #[clap(long, env)]
    pub blocknative_api_key: Option<String>,

    /// Base tokens used for finding multi-hop paths between multiple AMMs
    /// Should be the most liquid tokens of the given network.
    #[clap(long, env, use_value_delimiter = true)]
    pub base_tokens: Vec<H160>,

    /// Which Liquidity sources to be used by Price Estimator.
    #[clap(long, env, arg_enum, ignore_case = true, use_value_delimiter = true)]
    pub baseline_sources: Option<Vec<BaselineSource>>,

    /// The number of blocks kept in the pool cache.
    #[clap(long, env, default_value = "10")]
    pub pool_cache_blocks: NonZeroU64,

    /// The number of pairs that are automatically updated in the pool cache.
    #[clap(long, env, default_value = "4")]
    pub pool_cache_maximum_recent_block_age: u64,

    /// How often to retry requests in the pool cache.
    #[clap(long, env, default_value = "5")]
    pub pool_cache_maximum_retries: u32,

    /// How long to sleep in seconds between retries in the pool cache.
    #[clap(long, env, default_value = "1", parse(try_from_str = duration_from_seconds))]
    pub pool_cache_delay_between_retries_seconds: Duration,

    /// How often in seconds we poll the node to check if the current block has changed.
    #[clap(
        long,
        env,
        default_value = "5",
        parse(try_from_str = duration_from_seconds),
    )]
    pub block_stream_poll_interval_seconds: Duration,

    /// The Balancer V2 factories to consider for indexing liquidity. Allows
    /// specific pool kinds to be disabled via configuration. Will use all
    /// supported Balancer V2 factory kinds if not specified.
    #[clap(long, env, arg_enum, ignore_case = true, use_value_delimiter = true)]
    pub balancer_factories: Option<Vec<BalancerFactoryKind>>,

    /// Deny list of balancer pool ids.
    #[clap(long, env, use_value_delimiter = true)]
    pub balancer_pool_deny_list: Vec<H256>,

    /// If liquidity pool fetcher has caching mechanism, this argument defines how old pool data is allowed
    /// to be before updating
    #[clap(
        long,
        default_value = "30",
        parse(try_from_str = duration_from_seconds),
    )]
    pub liquidity_fetcher_max_age_update: Duration,

    /// ZeroEx API endpoint URL.
    #[clap(long, env)]
    pub zeroex_url: Option<String>,

    /// ZeroEx API key.
    #[clap(long, env)]
    pub zeroex_api_key: Option<String>,
}

impl std::fmt::Display for Arguments {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "bind_address: {}", self.bind_address)?;
        writeln!(f, "log_filter: {}", self.log_filter)?;
        writeln!(f, "log_stderr_threshold: {}", self.log_stderr_threshold)?;
        writeln!(f, "solvers: {:?}", self.solvers)?;
        writeln!(f, "node_url: {}", self.node_url)?;
        writeln!(f, "http_timeout: {:?}", self.http_timeout)?;
        writeln!(f, "use_internal_buffers: {}", self.use_internal_buffers)?;
        write!(f, "transaction_submission_nodes: ")?;
        display_list(self.transaction_submission_nodes.iter(), f)?;
        writeln!(f)?;
        writeln!(
            f,
            "disable_high_risk_public_mempool_transactions: {}",
            self.disable_high_risk_public_mempool_transactions,
        )?;
        writeln!(
            f,
            "fee_objective_scaling_factor: {}",
            self.fee_objective_scaling_factor,
        )?;
        writeln!(f, "transaction_strategy: {:?}", self.transaction_strategy)?;
        writeln!(f, "eden_api_url: {}", self.eden_api_url)?;
        writeln!(
            f,
            "max_additional_eden_tip: {}",
            self.max_additional_eden_tip
        )?;
        writeln!(
            f,
            "additional_tip_percentage: {}",
            self.additional_tip_percentage
        )?;
        write!(f, "flashbots_api_url: ")?;
        display_list(self.flashbots_api_url.iter(), f)?;
        writeln!(f)?;
        writeln!(
            f,
            "max_additional_flashbots_tip: {}",
            self.max_additional_flashbot_tip
        )?;
        writeln!(
            f,
            "access_list_estimators: {:?}",
            self.access_list_estimators
        )?;
        write!(f, "tenderly_url: ")?;
        display_option(&self.tenderly_url, f)?;
        writeln!(f)?;
        writeln!(
            f,
            "tenderly_api_key: {}",
            self.tenderly_api_key
                .as_deref()
                .map(|_| "SECRET")
                .unwrap_or("None")
        )?;
        writeln!(f, "simulation_gas_limit: {}", self.simulation_gas_limit)?;
        writeln!(f, "target_confirm_time: {:?}", self.target_confirm_time)?;
        writeln!(
            f,
            "max_submission_seconds: {:?}",
            self.max_submission_seconds
        )?;
        writeln!(
            f,
            "submission_retry_interval_seconds: {:?}",
            self.submission_retry_interval_seconds
        )?;
        writeln!(f, "gas_price_cap: {}", self.gas_price_cap)?;
        writeln!(f, "gas_estimators: {:?}", self.gas_estimators)?;
        writeln!(
            f,
            "blocknative_api_key: {}",
            self.blocknative_api_key
                .as_ref()
                .map(|_| "SECRET")
                .unwrap_or("None")
        )?;
        writeln!(f, "base_tokens: {:?}", self.base_tokens)?;
        writeln!(f, "baseline_sources: {:?}", self.baseline_sources)?;
        writeln!(f, "pool_cache_blocks: {}", self.pool_cache_blocks)?;
        writeln!(
            f,
            "pool_cache_maximum_recent_block_age: {}",
            self.pool_cache_maximum_recent_block_age
        )?;
        writeln!(
            f,
            "pool_cache_maximum_retries: {}",
            self.pool_cache_maximum_retries
        )?;
        writeln!(
            f,
            "pool_cache_delay_between_retries_seconds: {:?}",
            self.pool_cache_delay_between_retries_seconds
        )?;
        writeln!(
            f,
            "block_stream_poll_interval_seconds: {:?}",
            self.block_stream_poll_interval_seconds
        )?;
        writeln!(f, "balancer_factories: {:?}", self.balancer_factories)?;
        writeln!(
            f,
            "balancer_pool_deny_list: {:?}",
            self.balancer_pool_deny_list
        )?;
        writeln!(
            f,
            "liquidity_fetcher_max_age_update: {:?}",
            self.liquidity_fetcher_max_age_update
        )?;
        writeln!(
            f,
            "zeroex_url: {}",
            self.zeroex_url.as_deref().unwrap_or("None")
        )?;
        writeln!(
            f,
            "zeroex_api_key: {}",
            self.zeroex_api_key
                .as_ref()
                .map(|_| "SECRET")
                .unwrap_or("None")
        )?;
        Ok(())
    }
}
