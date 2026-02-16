use {
    alloy::primitives::Address,
    chrono::{DateTime, Utc},
    reqwest::Url,
    shared::{
        arguments::{FeeFactor, display_secret_option},
        http_client,
        price_estimation::{self, NativePriceEstimators},
    },
    std::{net::SocketAddr, num::NonZeroUsize, time::Duration},
};

#[derive(clap::Parser)]
pub struct Arguments {
    #[clap(flatten)]
    pub shared: shared::arguments::Arguments,

    #[clap(flatten)]
    pub order_quoting: shared::arguments::OrderQuotingArguments,

    #[clap(flatten)]
    pub http_client: http_client::Arguments,

    #[clap(flatten)]
    pub price_estimation: price_estimation::Arguments,

    #[clap(flatten)]
    pub database_pool: shared::arguments::DatabasePoolConfig,

    #[clap(long, env, default_value = "0.0.0.0:8080")]
    pub bind_address: SocketAddr,

    /// Url of the Postgres database. By default connects to locally running
    /// postgres.
    #[clap(long, env, default_value = "postgresql://")]
    pub db_write_url: Url,

    /// Url of the Postgres database replica. By default it's the same as
    /// db_write_url
    #[clap(long, env)]
    pub db_read_url: Option<Url>,

    /// The minimum amount of time in seconds an order has to be valid for.
    #[clap(
        long,
        env,
        default_value = "1m",
        value_parser = humantime::parse_duration,
    )]
    pub min_order_validity_period: Duration,

    /// The maximum amount of time in seconds an order can be valid for.
    /// Defaults to 3 hours. This restriction does not apply to liquidity
    /// owner orders or presign orders.
    #[clap(
        long,
        env,
        default_value = "3h",
        value_parser = humantime::parse_duration,
    )]
    pub max_order_validity_period: Duration,

    /// The maximum amount of time in seconds a limit order can be valid for.
    /// Defaults to 1 year.
    #[clap(
        long,
        env,
        default_value = "1y",
        value_parser = humantime::parse_duration,
    )]
    pub max_limit_order_validity_period: Duration,

    /// List of token addresses to be ignored throughout service
    #[clap(long, env, use_value_delimiter = true)]
    pub unsupported_tokens: Vec<Address>,

    /// List of account addresses to be denied from order creation
    #[clap(long, env, use_value_delimiter = true)]
    pub banned_users: Vec<Address>,

    /// Maximum number of entries to keep in the banned users cache.
    #[clap(long, env, default_value = "100")]
    pub banned_users_max_cache_size: NonZeroUsize,

    /// Which estimators to use to estimate token prices in terms of the chain's
    /// native token.
    #[clap(long, env)]
    pub native_price_estimators: NativePriceEstimators,

    /// Fallback native price estimators to use when all primary estimators
    /// are down.
    #[clap(long, env)]
    pub fallback_native_price_estimators: Option<NativePriceEstimators>,

    /// How many successful price estimates for each order will cause a fast
    /// or native price estimation to return its result early.
    /// The bigger the value the more the fast price estimation performs like
    /// the optimal price estimation.
    /// It's possible to pass values greater than the total number of enabled
    /// estimators but that will not have any further effect.
    #[clap(long, env, default_value = "2")]
    pub fast_price_estimation_results_required: NonZeroUsize,

    /// List of token addresses that should be allowed regardless of whether the
    /// bad token detector thinks they are bad. Base tokens are
    /// automatically allowed.
    #[clap(long, env, use_value_delimiter = true)]
    pub allowed_tokens: Vec<Address>,

    /// Skip EIP-1271 order signature validation on creation.
    #[clap(long, env, action = clap::ArgAction::Set, default_value = "false")]
    pub eip1271_skip_creation_validation: bool,

    /// Max number of limit orders per user.
    #[clap(long, env, default_value = "10")]
    pub max_limit_orders_per_user: u64,

    /// If set, the orderbook will use this IPFS gateway to fetch full app data
    /// for orders that only specify the contract app data hash.
    #[clap(long, env)]
    pub ipfs_gateway: Option<Url>,

    /// Authentication key for Pinata IPFS gateway.
    #[clap(long, env)]
    pub ipfs_pinata_auth: Option<String>,

    /// Set the maximum size in bytes of order app data.
    #[clap(long, env, default_value = "8192")]
    pub app_data_size_limit: usize,

    /// The maximum gas amount a single order can use for getting settled.
    #[clap(long, env, default_value = "8000000")]
    pub max_gas_per_order: u64,

    /// The number of past solver competitions to look back at to determine
    /// whether an order is actively being bid on.
    #[clap(long, env, default_value = "5")]
    pub active_order_competition_threshold: u32,

    #[clap(flatten)]
    pub volume_fee_config: Option<VolumeFeeConfig>,

    /// Controls if same sell and buy token orders are allowed.
    /// Disallowed by default.
    #[clap(long, env, default_value = "disallow")]
    pub same_tokens_policy: shared::order_validation::SameTokensPolicy,
}

/// Volume-based protocol fee factor to be applied to quotes.
#[derive(clap::Parser, Debug, Clone)]
pub struct VolumeFeeConfig {
    /// This is a decimal value (e.g., 0.0002 for 0.02% or 2 basis points).
    /// The fee is applied to the surplus token (buy token for sell orders,
    /// sell token for buy orders).
    #[clap(
        id = "volume_fee_factor",
        long = "volume-fee-factor",
        env = "VOLUME_FEE_FACTOR"
    )]
    pub factor: Option<FeeFactor>,

    /// The timestamp from which the volume fee becomes effective.
    #[clap(
        long = "volume-fee-effective-timestamp",
        env = "VOLUME_FEE_EFFECTIVE_TIMESTAMP"
    )]
    pub effective_from_timestamp: Option<DateTime<Utc>>,
}

impl std::fmt::Display for Arguments {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        let Arguments {
            shared,
            order_quoting,
            http_client,
            price_estimation,
            database_pool,
            bind_address,
            min_order_validity_period,
            max_order_validity_period,
            max_limit_order_validity_period,
            unsupported_tokens,
            banned_users,
            banned_users_max_cache_size,
            allowed_tokens,
            eip1271_skip_creation_validation,
            native_price_estimators,
            fallback_native_price_estimators,
            fast_price_estimation_results_required,
            max_limit_orders_per_user,
            ipfs_gateway,
            ipfs_pinata_auth,
            app_data_size_limit,
            db_write_url: db_url,
            db_read_url,
            max_gas_per_order,
            active_order_competition_threshold,
            volume_fee_config,
            same_tokens_policy,
        } = self;

        write!(f, "{shared}")?;
        write!(f, "{order_quoting}")?;
        write!(f, "{http_client}")?;
        write!(f, "{price_estimation}")?;
        write!(f, "{database_pool}")?;
        writeln!(f, "bind_address: {bind_address}")?;
        let _intentionally_ignored = db_url;
        writeln!(f, "db_url: SECRET")?;
        display_secret_option(f, "db_read_url", db_read_url.as_ref())?;
        writeln!(
            f,
            "min_order_validity_period: {min_order_validity_period:?}"
        )?;
        writeln!(
            f,
            "max_order_validity_period: {max_order_validity_period:?}"
        )?;
        writeln!(
            f,
            "max_limit_order_validity_period: {max_limit_order_validity_period:?}"
        )?;
        writeln!(f, "unsupported_tokens: {unsupported_tokens:?}")?;
        writeln!(f, "banned_users: {banned_users:?}")?;
        writeln!(
            f,
            "banned_users_max_cache_size: {banned_users_max_cache_size:?}"
        )?;
        writeln!(f, "allowed_tokens: {allowed_tokens:?}")?;
        writeln!(
            f,
            "eip1271_skip_creation_validation: {eip1271_skip_creation_validation}"
        )?;
        writeln!(f, "native_price_estimators: {native_price_estimators}")?;
        writeln!(
            f,
            "fallback_native_price_estimators: {fallback_native_price_estimators:?}"
        )?;
        writeln!(
            f,
            "fast_price_estimation_results_required: {fast_price_estimation_results_required}"
        )?;
        writeln!(f, "max_limit_orders_per_user: {max_limit_orders_per_user}")?;
        writeln!(f, "ipfs_gateway: {ipfs_gateway:?}")?;
        display_secret_option(f, "ipfs_pinata_auth", ipfs_pinata_auth.as_ref())?;
        writeln!(f, "app_data_size_limit: {app_data_size_limit}")?;
        writeln!(f, "max_gas_per_order: {max_gas_per_order}")?;
        writeln!(
            f,
            "active_order_competition_threshold: {active_order_competition_threshold}"
        )?;
        writeln!(f, "volume_fee_config: {volume_fee_config:?}")?;
        writeln!(f, "same_tokens_policy: {same_tokens_policy:?}")?;

        Ok(())
    }
}
