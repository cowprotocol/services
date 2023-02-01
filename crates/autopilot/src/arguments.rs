use crate::limit_orders::QuotingStrategy;
use primitive_types::H160;
use shared::{
    arguments::display_option, bad_token::token_owner_finder, http_client, price_estimation,
};
use std::{net::SocketAddr, num::NonZeroUsize, time::Duration};
use url::Url;

#[derive(clap::Parser)]
pub struct Arguments {
    #[clap(flatten)]
    pub shared: shared::arguments::Arguments,

    #[clap(flatten)]
    pub order_quoting: shared::arguments::OrderQuotingArguments,

    #[clap(flatten)]
    pub http_client: http_client::Arguments,

    #[clap(flatten)]
    pub token_owner_finder: token_owner_finder::Arguments,

    #[clap(flatten)]
    pub price_estimation: price_estimation::Arguments,

    /// Address of the ethflow contract. If not specified, eth-flow orders are disabled.
    #[clap(long, env)]
    pub ethflow_contract: Option<H160>,

    /// Timestamp at which we should start indexing eth-flow contract events.
    /// If there are already events in the database for a date later than this, then this date is
    /// ignored and can be omitted.
    #[clap(long, env)]
    pub ethflow_indexing_start: Option<u64>,

    /// A tracing Ethereum node URL to connect to, allowing a separate node URL
    /// to be used exclusively for tracing calls.
    #[clap(long, env)]
    pub tracing_node_url: Option<Url>,

    #[clap(long, env, default_value = "0.0.0.0:9589")]
    pub metrics_address: SocketAddr,

    /// Url of the Postgres database. By default connects to locally running postgres.
    #[clap(long, env, default_value = "postgresql://")]
    pub db_url: Url,

    /// Skip syncing past events (useful for local deployments)
    #[clap(long, env)]
    pub skip_event_sync: bool,

    /// List of token addresses that should be allowed regardless of whether the bad token detector
    /// thinks they are bad. Base tokens are automatically allowed.
    #[clap(long, env, use_value_delimiter = true)]
    pub allowed_tokens: Vec<H160>,

    /// List of token addresses to be ignored throughout service
    #[clap(long, env, use_value_delimiter = true)]
    pub unsupported_tokens: Vec<H160>,

    /// The amount of time in seconds a classification of a token into good or bad is valid for.
    #[clap(
        long,
        env,
        default_value = "600",
        value_parser = shared::arguments::duration_from_seconds,
    )]
    pub token_quality_cache_expiry: Duration,

    /// The number of pairs that are automatically updated in the pool cache.
    #[clap(long, env, default_value = "200")]
    pub pool_cache_lru_size: NonZeroUsize,

    /// Which estimators to use to estimate token prices in terms of the chain's native token.
    #[clap(
        long,
        env,
        default_value = "Baseline",
        value_enum,
        use_value_delimiter = true
    )]
    pub native_price_estimators: Vec<shared::price_estimation::PriceEstimatorType>,

    /// The minimum amount of time in seconds an order has to be valid for.
    #[clap(
        long,
        env,
        default_value = "60",
        value_parser = shared::arguments::duration_from_seconds,
    )]
    pub min_order_validity_period: Duration,

    /// List of account addresses to be denied from order creation
    #[clap(long, env, use_value_delimiter = true)]
    pub banned_users: Vec<H160>,

    /// If the auction hasn't been updated in this amount of time the pod fails the liveness check.
    /// Expects a value in seconds.
    #[clap(
        long,
        env,
        default_value = "300",
        value_parser = shared::arguments::duration_from_seconds,
    )]
    pub max_auction_age: Duration,

    /// If a limit order surplus fee is older than this, it will get refreshed. Expects a value in
    /// seconds.
    #[clap(
        long,
        env,
        default_value = "180",
        value_parser = shared::arguments::duration_from_seconds,
    )]
    pub max_surplus_fee_age: Duration,

    #[clap(long, env)]
    pub cip_14_beta: Option<f64>,
    #[clap(long, env)]
    pub cip_14_alpha1: Option<f64>,
    #[clap(long, env)]
    pub cip_14_alpha2: Option<f64>,
    /// in COW base units
    #[clap(long, env)]
    pub cip_14_profit: Option<f64>,
    /// in gas units
    #[clap(long, env)]
    pub cip_14_gas_cap: Option<f64>,
    /// in COW base units
    #[clap(long, env)]
    pub cip_14_reward_cap: Option<f64>,

    #[clap(long, env, default_value = "0")]
    pub limit_order_price_factor: f64,

    /// Enable background quoting for limit orders.
    #[clap(long, env)]
    pub enable_limit_orders: bool,

    /// How many quotes the limit order quoter updates in parallel.
    #[clap(long, env, default_value = "5")]
    pub limit_order_quoter_parallelism: usize,

    /// How many quotes the limit order quoter updates per update cycle.
    #[clap(long, env, default_value = "25")]
    pub limit_order_quoter_batch_size: usize,

    /// The time between auction updates.
    #[clap(long, env, default_value = "10", value_parser = shared::arguments::duration_from_seconds)]
    pub auction_update_interval: Duration,

    /// The time in seconds between new blocks on the network.
    #[clap(long, env, value_parser = shared::arguments::duration_from_seconds)]
    pub network_block_interval: Option<Duration>,

    /// Defines which strategies to apply when updating the `surplus_fee` of limit orders.
    #[clap(long, env, use_value_delimiter = true)]
    pub quoting_strategies: Vec<QuotingStrategy>,
}

impl std::fmt::Display for Arguments {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.shared)?;
        write!(f, "{}", self.order_quoting)?;
        write!(f, "{}", self.http_client)?;
        write!(f, "{}", self.token_owner_finder)?;
        write!(f, "{}", self.price_estimation)?;
        display_option(f, "tracing_node_url", &self.tracing_node_url)?;
        writeln!(f, "ethflow_contract: {:?}", self.ethflow_contract)?;
        writeln!(
            f,
            "ethflow_indexing_start: {:?}",
            self.ethflow_indexing_start
        )?;
        writeln!(f, "metrics_address: {}", self.metrics_address)?;
        writeln!(f, "db_url: SECRET")?;
        writeln!(f, "skip_event_sync: {}", self.skip_event_sync)?;
        writeln!(f, "allowed_tokens: {:?}", self.allowed_tokens)?;
        writeln!(f, "unsupported_tokens: {:?}", self.unsupported_tokens)?;
        writeln!(
            f,
            "token_quality_cache_expiry: {:?}",
            self.token_quality_cache_expiry
        )?;
        writeln!(f, "pool_cache_lru_size: {}", self.pool_cache_lru_size)?;
        writeln!(
            f,
            "native_price_estimators: {:?}",
            self.native_price_estimators
        )?;
        writeln!(
            f,
            "min_order_validity_period: {:?}",
            self.min_order_validity_period
        )?;
        writeln!(f, "banned_users: {:?}", self.banned_users)?;
        writeln!(f, "max_auction_age: {:?}", self.max_auction_age)?;
        writeln!(f, "max_surplus_fee_age: {:?}", self.max_surplus_fee_age)?;
        display_option(f, "cip_14_beta", &self.cip_14_beta)?;
        display_option(f, "cip_14_alpha1", &self.cip_14_alpha1)?;
        display_option(f, "cip_14_alpha2", &self.cip_14_alpha2)?;
        display_option(f, "cip_14_profit", &self.cip_14_profit)?;
        display_option(f, "cip_14_gas_cap", &self.cip_14_gas_cap)?;
        display_option(f, "cip_14_reward_cap", &self.cip_14_reward_cap)?;
        writeln!(
            f,
            "limit_order_price_factor: {:?}",
            self.limit_order_price_factor
        )?;
        writeln!(f, "enable_limit_orders: {:?}", self.enable_limit_orders)?;
        writeln!(
            f,
            "limit_order_quoter_parallelism: {:?}",
            self.limit_order_quoter_parallelism
        )?;
        writeln!(
            f,
            "limit_order_quoter_batch_size: {:?}",
            self.limit_order_quoter_batch_size,
        )?;
        display_option(
            f,
            "network_block_interval",
            &self
                .network_block_interval
                .map(|duration| duration.as_secs_f32()),
        )?;
        writeln!(f, "quoting_strategies: {:?}", self.quoting_strategies)?;
        Ok(())
    }
}
