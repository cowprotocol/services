use primitive_types::{H160, U256};
use shared::{arguments::display_option, bad_token::token_owner_finder::FeeValues};
use std::{net::SocketAddr, time::Duration};
use url::Url;

#[derive(clap::Parser)]
pub struct Arguments {
    #[clap(flatten)]
    pub shared: shared::arguments::Arguments,

    #[clap(long, env, default_value = "0.0.0.0:9589")]
    pub metrics_address: SocketAddr,

    /// Url of the Postgres database. By default connects to locally running postgres.
    #[clap(long, env, default_value = "postgresql://")]
    pub db_url: Url,

    /// Skip syncing past events (useful for local deployments)
    #[clap(long)]
    pub skip_event_sync: bool,

    /// List of token addresses that should be allowed regardless of whether the bad token detector
    /// thinks they are bad. Base tokens are automatically allowed.
    #[clap(long, env, use_value_delimiter = true)]
    pub allowed_tokens: Vec<H160>,

    /// List of token addresses to be ignored throughout service
    #[clap(long, env, use_value_delimiter = true)]
    pub unsupported_tokens: Vec<H160>,

    #[clap(long, env, default_value = "static", arg_enum)]
    pub token_detector_fee_values: FeeValues,

    /// Use Blockscout as a TokenOwnerFinding implementation.
    #[clap(long, env, default_value = "true")]
    pub enable_blockscout: bool,

    /// The amount of time in seconds a classification of a token into good or bad is valid for.
    #[clap(
        long,
        env,
        default_value = "600",
        parse(try_from_str = shared::arguments::duration_from_seconds),
    )]
    pub token_quality_cache_expiry: Duration,

    /// Don't use the trace_callMany api that only some nodes support to check whether a token
    /// should be denied.
    /// Note that if a node does not support the api we still use the less accurate call api.
    #[clap(long, env, parse(try_from_str), default_value = "false")]
    pub skip_trace_api: bool,

    /// The number of pairs that are automatically updated in the pool cache.
    #[clap(long, env, default_value = "200")]
    pub pool_cache_lru_size: usize,

    /// The API endpoint for the Balancer SOR API for solving.
    #[clap(long, env)]
    pub balancer_sor_url: Option<Url>,

    /// Configures the back off strategy for price estimators when requests take too long.
    /// Requests issued while back off is active get dropped entirely.
    /// Needs to be passed as "<back_off_growth_factor>,<min_back_off>,<max_back_off>".
    /// back_off_growth_factor: f64 >= 1.0
    /// min_back_off: f64 in seconds
    /// max_back_off: f64 in seconds
    #[clap(long, env, verbatim_doc_comment)]
    pub price_estimation_rate_limiter: Option<shared::rate_limiter::RateLimitingStrategy>,

    /// The amount in native tokens atoms to use for price estimation. Should be reasonably large so
    /// that small pools do not influence the prices. If not set a reasonable default is used based
    /// on network id.
    #[clap(
        long,
        env,
        parse(try_from_str = U256::from_dec_str)
    )]
    pub amount_to_estimate_prices_with: Option<U256>,

    /// The API endpoint to call the mip v2 solver for price estimation
    #[clap(long, env)]
    pub quasimodo_solver_url: Option<Url>,

    /// The API endpoint to call the yearn solver for price estimation
    #[clap(long, env)]
    pub yearn_solver_url: Option<Url>,

    /// Which estimators to use to estimate token prices in terms of the chain's native token.
    #[clap(
        long,
        env,
        default_value = "Baseline",
        arg_enum,
        use_value_delimiter = true
    )]
    pub native_price_estimators: Vec<shared::price_estimation::PriceEstimatorType>,

    /// How long cached native prices stay valid.
    #[clap(
        long,
        env,
        default_value = "30",
        parse(try_from_str = shared::arguments::duration_from_seconds),
    )]
    pub native_price_cache_max_age_secs: Duration,
}

impl std::fmt::Display for Arguments {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.shared)?;
        writeln!(f, "metrics_address: {}", self.metrics_address)?;
        writeln!(f, "db_url: SECRET")?;
        writeln!(f, "skip_event_sync: {}", self.skip_event_sync)?;
        writeln!(f, "allowed_tokens: {:?}", self.allowed_tokens)?;
        writeln!(f, "unsupported_tokens: {:?}", self.unsupported_tokens)?;
        writeln!(
            f,
            "token_detector_fee_values: {:?}",
            self.token_detector_fee_values
        )?;
        writeln!(f, "enable_blockscout: {}", self.enable_blockscout)?;
        writeln!(
            f,
            "token_quality_cache_expiry: {:?}",
            self.token_quality_cache_expiry
        )?;
        writeln!(f, "skip_trace_api: {}", self.skip_trace_api)?;
        writeln!(f, "pool_cache_lru_size: {}", self.pool_cache_lru_size)?;
        write!(f, "balancer_sor_url: ")?;
        write!(f, "price_estimation_rate_limiter: ")?;
        display_option(&self.price_estimation_rate_limiter, f)?;
        writeln!(f)?;
        write!(f, "amount_to_estimate_prices_with: ")?;
        display_option(&self.amount_to_estimate_prices_with, f)?;
        writeln!(f)?;
        write!(f, "quasimodo_solver_url: ")?;
        display_option(&self.quasimodo_solver_url, f)?;
        writeln!(f)?;
        write!(f, "yearn_solver_url: ")?;
        display_option(&self.yearn_solver_url, f)?;
        writeln!(f)?;
        writeln!(
            f,
            "native_price_estimators: {:?}",
            self.native_price_estimators
        )?;
        writeln!(
            f,
            "native_price_cache_max_age_secs: {:?}",
            self.native_price_cache_max_age_secs
        )?;
        Ok(())
    }
}
