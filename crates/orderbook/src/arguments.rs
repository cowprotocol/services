use primitive_types::H160;
use reqwest::Url;
use shared::{
    arguments::display_option,
    bad_token::token_owner_finder,
    http_client,
    price_estimation::{self, PriceEstimatorType},
};
use std::{net::SocketAddr, num::NonZeroUsize, time::Duration};

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

    /// A tracing Ethereum node URL to connect to, allowing a separate node URL
    /// to be used exclusively for tracing calls.
    #[clap(long, env)]
    pub tracing_node_url: Option<Url>,

    #[clap(long, env, default_value = "0.0.0.0:8080")]
    pub bind_address: SocketAddr,

    /// Url of the Postgres database. By default connects to locally running postgres.
    #[clap(long, env, default_value = "postgresql://")]
    pub db_url: Url,

    /// The minimum amount of time in seconds an order has to be valid for.
    #[clap(
        long,
        env,
        default_value = "60",
        value_parser = shared::arguments::duration_from_seconds,
    )]
    pub min_order_validity_period: Duration,

    /// The maximum amount of time in seconds an order can be valid for. Defaults to 3 hours. This
    /// restriction does not apply to liquidity owner orders or presign orders.
    #[clap(
        long,
        env,
        default_value = "10800",
        value_parser = shared::arguments::duration_from_seconds,
    )]
    pub max_order_validity_period: Duration,

    /// The amount of time in seconds a classification of a token into good or bad is valid for.
    #[clap(
        long,
        env,
        default_value = "600",
        value_parser = shared::arguments::duration_from_seconds,
    )]
    pub token_quality_cache_expiry: Duration,

    /// List of token addresses to be ignored throughout service
    #[clap(long, env, use_value_delimiter = true)]
    pub unsupported_tokens: Vec<H160>,

    /// List of account addresses to be denied from order creation
    #[clap(long, env, use_value_delimiter = true)]
    pub banned_users: Vec<H160>,

    /// Which estimators to use to estimate token prices in terms of the chain's native token.
    #[clap(
        long,
        env,
        default_value = "Baseline",
        value_enum,
        use_value_delimiter = true
    )]
    pub native_price_estimators: Vec<PriceEstimatorType>,

    /// How many successful price estimates for each order will cause a fast price estimation to
    /// return its result early.
    /// The bigger the value the more the fast price estimation performs like the optimal price
    /// estimation.
    /// It's possible to pass values greater than the total number of enabled estimators but that
    /// will not have any further effect.
    #[clap(long, env, default_value = "2")]
    pub fast_price_estimation_results_required: NonZeroUsize,

    /// List of token addresses that should be allowed regardless of whether the bad token detector
    /// thinks they are bad. Base tokens are automatically allowed.
    #[clap(long, env, use_value_delimiter = true)]
    pub allowed_tokens: Vec<H160>,

    /// The number of pairs that are automatically updated in the pool cache.
    #[clap(long, env, default_value = "200")]
    pub pool_cache_lru_size: NonZeroUsize,

    /// Enable EIP-1271 orders.
    #[clap(long, env)]
    pub enable_eip1271_orders: bool,

    /// Skip EIP-1271 order signature validation on creation.
    #[clap(long, env)]
    pub eip1271_skip_creation_validation: bool,

    /// Enable pre-sign orders. Pre-sign orders are accepted into the database without a valid
    /// signature, so this flag allows this feature to be turned off if malicious users are
    /// abusing the database by inserting a bunch of order rows that won't ever be valid.
    /// This flag can be removed once DDoS protection is implemented.
    #[clap(long, env)]
    pub enable_presign_orders: bool,

    /// If solvable orders haven't been successfully updated in this many blocks attempting
    /// to get them errors and our liveness check fails.
    #[clap(long, env, default_value = "24")]
    pub solvable_orders_max_update_age_blocks: u64,

    /// Enable limit orders. Once the full limit order flow is implemented, this can be removed.
    #[clap(long, env, default_value = "false")]
    pub enable_limit_orders: bool,

    /// Max number of limit orders per user.
    #[clap(long, env)]
    pub max_limit_orders_per_user: u64,
}

impl std::fmt::Display for Arguments {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "{}", self.shared)?;
        write!(f, "{}", self.order_quoting)?;
        write!(f, "{}", self.http_client)?;
        write!(f, "{}", self.token_owner_finder)?;
        write!(f, "{}", self.price_estimation)?;
        display_option(f, "tracing_node_url", &self.tracing_node_url)?;
        writeln!(f, "bind_address: {}", self.bind_address)?;
        writeln!(f, "db_url: SECRET")?;
        writeln!(
            f,
            "min_order_validity_period: {:?}",
            self.min_order_validity_period
        )?;
        writeln!(
            f,
            "max_order_validity_period: {:?}",
            self.max_order_validity_period
        )?;
        writeln!(
            f,
            "token_quality_cache_expiry: {:?}",
            self.token_quality_cache_expiry
        )?;
        writeln!(f, "unsupported_tokens: {:?}", self.unsupported_tokens)?;
        writeln!(f, "banned_users: {:?}", self.banned_users)?;
        writeln!(f, "allowed_tokens: {:?}", self.allowed_tokens)?;
        writeln!(f, "pool_cache_lru_size: {}", self.pool_cache_lru_size)?;
        writeln!(f, "enable_eip1271_orders: {}", self.enable_eip1271_orders)?;
        writeln!(
            f,
            "eip1271_skip_creation_validation: {}",
            self.eip1271_skip_creation_validation
        )?;
        writeln!(f, "enable_presign_orders: {}", self.enable_presign_orders)?;
        writeln!(
            f,
            "solvable_orders_max_update_age_blocks: {}",
            self.solvable_orders_max_update_age_blocks,
        )?;
        writeln!(
            f,
            "native_price_estimators: {:?}",
            self.native_price_estimators
        )?;
        writeln!(
            f,
            "fast_price_estimation_results_required: {}",
            self.fast_price_estimation_results_required
        )?;
        writeln!(f, "enable_limit_orders: {}", self.enable_limit_orders)?;

        Ok(())
    }
}
