//! Contains command line arguments and related helpers that are shared between
//! the binaries.

use {
    alloy::primitives::Address,
    configs::fee_factor::FeeFactor,
    gas_price_estimation::GasEstimatorType,
    observe::TracingConfig,
    std::{
        collections::HashSet,
        fmt::{self, Display, Formatter},
        num::NonZeroU32,
        time::Duration,
    },
};

#[macro_export]
macro_rules! logging_args_with_default_filter {
    ($struct_name:ident ,$default_filter:literal) => {
        #[derive(clap::Parser)]
        pub struct $struct_name {
            #[clap(long, env, default_value = $default_filter)]
            pub log_filter: String,

            #[clap(long, env)]
            pub log_stderr_threshold: Option<tracing::Level>,

            #[clap(long, env, default_value = "false")]
            pub use_json_logs: bool,
        }

        impl ::std::fmt::Display for $struct_name {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                let Self {
                    log_filter,
                    log_stderr_threshold,
                    use_json_logs,
                } = self;

                writeln!(f, "log_filter: {}", log_filter)?;
                writeln!(f, "log_stderr_threshold: {:?}", log_stderr_threshold)?;
                writeln!(f, "use_json_logs: {}", use_json_logs)?;
                Ok(())
            }
        }
    };
}

logging_args_with_default_filter!(
    LoggingArguments,
    "info,autopilot=debug,driver=debug,observe=info,orderbook=debug,solver=debug,shared=debug,\
     cow_amm=debug"
);

#[derive(Debug, clap::Parser)]
pub struct TracingArguments {
    #[clap(long, env)]
    pub tracing_collector_endpoint: Option<String>,
    #[clap(long, env, default_value_t = tracing::Level::INFO)]
    pub tracing_level: tracing::Level,
    #[clap(long, env, value_parser = humantime::parse_duration, default_value = "10s")]
    pub tracing_exporter_timeout: Duration,
}

pub fn tracing_config(args: &TracingArguments, service_name: String) -> Option<TracingConfig> {
    let Some(endpoint) = &args.tracing_collector_endpoint else {
        return None;
    };

    Some(TracingConfig::new(
        endpoint.clone(),
        service_name,
        args.tracing_exporter_timeout,
        args.tracing_level,
    ))
}

// Matches SQLx default connection pool size.
// SAFETY: 10 > 0
pub const DB_MAX_CONNECTIONS_DEFAULT: NonZeroU32 = NonZeroU32::new(10).unwrap();

#[derive(Debug, Clone, clap::Parser)]
pub struct DatabasePoolConfig {
    /// Maximum number of connections in the database connection pool.
    #[clap(long, env, default_value_t = DB_MAX_CONNECTIONS_DEFAULT)]
    pub db_max_connections: NonZeroU32,

    /// Timeout for database read queries.
    #[clap(long, env, default_value = "30s", value_parser = humantime::parse_duration)]
    pub global_query_timeout: Duration,
}

impl Display for DatabasePoolConfig {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        writeln!(f, "db_max_connections: {}", self.db_max_connections)?;
        writeln!(f, "global_query_timeout: {:?}", self.global_query_timeout)
    }
}

pub fn display_option(
    f: &mut Formatter<'_>,
    name: &str,
    option: &Option<impl Display>,
) -> std::fmt::Result {
    write!(f, "{name}: ")?;
    match option {
        Some(display) => writeln!(f, "{display}"),
        None => writeln!(f, "None"),
    }
}

/// Helper type for parsing token bucket fee overrides from strings
#[derive(Debug, Clone)]
pub struct TokenBucketFeeOverride {
    pub tokens: HashSet<Address>,
    pub factor: FeeFactor,
}

pub fn gas_estimator_type_from_config(
    config: &configs::shared::GasEstimatorType,
) -> GasEstimatorType {
    match config {
        configs::shared::GasEstimatorType::Web3 => GasEstimatorType::Web3,
        configs::shared::GasEstimatorType::Driver { url } => GasEstimatorType::Driver(url.clone()),
        configs::shared::GasEstimatorType::Alloy => GasEstimatorType::Alloy,
    }
}

impl From<&configs::shared::TokenBucketFeeOverride> for TokenBucketFeeOverride {
    fn from(config: &configs::shared::TokenBucketFeeOverride) -> Self {
        Self {
            tokens: config.tokens.clone(),
            factor: config.factor,
        }
    }
}
