use {
    primitive_types::H160,
    reqwest::Url,
    shared::{
        arguments::{display_option, display_secret_option},
        bad_token::token_owner_finder,
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
    pub token_owner_finder: token_owner_finder::Arguments,

    #[clap(flatten)]
    pub price_estimation: price_estimation::Arguments,

    /// A tracing Ethereum node URL to connect to, allowing a separate node URL
    /// to be used exclusively for tracing calls.
    #[clap(long, env)]
    pub tracing_node_url: Option<Url>,

    #[clap(long, env, default_value = "0.0.0.0:8080")]
    pub bind_address: SocketAddr,

    /// Url of the Postgres database. By default connects to locally running
    /// postgres.
    #[clap(long, env, default_value = "postgresql://")]
    pub db_url: Url,

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
    pub unsupported_tokens: Vec<H160>,

    /// List of account addresses to be denied from order creation
    #[clap(long, env, use_value_delimiter = true)]
    pub banned_users: Vec<H160>,

    /// Which estimators to use to estimate token prices in terms of the chain's
    /// native token.
    #[clap(long, env)]
    pub native_price_estimators: NativePriceEstimators,

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
    pub allowed_tokens: Vec<H160>,

    /// The number of pairs that are automatically updated in the pool cache.
    #[clap(long, env, default_value = "200")]
    pub pool_cache_lru_size: NonZeroUsize,

    /// Skip EIP-1271 order signature validation on creation.
    #[clap(long, env, action = clap::ArgAction::Set, default_value = "false")]
    pub eip1271_skip_creation_validation: bool,

    /// If solvable orders haven't been successfully updated in this many blocks
    /// attempting to get them errors and our liveness check fails.
    #[clap(long, env, default_value = "24")]
    pub solvable_orders_max_update_age_blocks: u64,

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

    /// Override the address of the `HooksTrampoline` contract used for
    /// trampolining custom order interactions. If not specified, the default
    /// contract deployment for the current network will be used.
    #[clap(long, env)]
    pub hooks_contract_address: Option<H160>,

    /// Set the maximum size in bytes of order app data.
    #[clap(long, env, default_value = "8192")]
    pub app_data_size_limit: usize,

    /// The maximum gas amount a single order can use for getting settled.
    #[clap(long, env, default_value = "8000000")]
    pub max_gas_per_order: u64,
}

impl std::fmt::Display for Arguments {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        let Arguments {
            shared,
            order_quoting,
            http_client,
            token_owner_finder,
            price_estimation,
            tracing_node_url,
            bind_address,
            min_order_validity_period,
            max_order_validity_period,
            max_limit_order_validity_period,
            unsupported_tokens,
            banned_users,
            allowed_tokens,
            pool_cache_lru_size,
            eip1271_skip_creation_validation,
            solvable_orders_max_update_age_blocks,
            native_price_estimators,
            fast_price_estimation_results_required,
            max_limit_orders_per_user,
            ipfs_gateway,
            ipfs_pinata_auth,
            hooks_contract_address,
            app_data_size_limit,
            db_url,
            max_gas_per_order,
        } = self;

        write!(f, "{}", shared)?;
        write!(f, "{}", order_quoting)?;
        write!(f, "{}", http_client)?;
        write!(f, "{}", token_owner_finder)?;
        write!(f, "{}", price_estimation)?;
        display_option(f, "tracing_node_url", tracing_node_url)?;
        writeln!(f, "bind_address: {}", bind_address)?;
        let _intentionally_ignored = db_url;
        writeln!(f, "db_url: SECRET")?;
        writeln!(
            f,
            "min_order_validity_period: {:?}",
            min_order_validity_period
        )?;
        writeln!(
            f,
            "max_order_validity_period: {:?}",
            max_order_validity_period
        )?;
        writeln!(
            f,
            "max_limit_order_validity_period: {:?}",
            max_limit_order_validity_period
        )?;
        writeln!(f, "unsupported_tokens: {:?}", unsupported_tokens)?;
        writeln!(f, "banned_users: {:?}", banned_users)?;
        writeln!(f, "allowed_tokens: {:?}", allowed_tokens)?;
        writeln!(f, "pool_cache_lru_size: {}", pool_cache_lru_size)?;
        writeln!(
            f,
            "eip1271_skip_creation_validation: {}",
            eip1271_skip_creation_validation
        )?;
        writeln!(
            f,
            "solvable_orders_max_update_age_blocks: {}",
            solvable_orders_max_update_age_blocks,
        )?;
        writeln!(f, "native_price_estimators: {}", native_price_estimators)?;
        writeln!(
            f,
            "fast_price_estimation_results_required: {}",
            fast_price_estimation_results_required
        )?;
        writeln!(
            f,
            "max_limit_orders_per_user: {}",
            max_limit_orders_per_user
        )?;
        writeln!(f, "ipfs_gateway: {:?}", ipfs_gateway)?;
        display_secret_option(f, "ipfs_pinata_auth", ipfs_pinata_auth.as_ref())?;
        display_option(
            f,
            "hooks_contract_address",
            &hooks_contract_address.map(|a| format!("{a:?}")),
        )?;
        writeln!(f, "app_data_size_limit: {}", app_data_size_limit)?;
        writeln!(f, "max_gas_per_order: {}", max_gas_per_order)?;

        Ok(())
    }
}
