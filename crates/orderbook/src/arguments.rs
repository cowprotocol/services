use {
    reqwest::Url,
    shared::{
        arguments::display_secret_option,
        http_client,
        price_estimation::{self, NativePriceEstimators},
    },
    std::{net::SocketAddr, num::NonZeroUsize, path::PathBuf},
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

    /// Path to the TOML configuration file.
    #[clap(long, env)]
    pub config: PathBuf,

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

    /// Which estimators to use to estimate token prices in terms of the chain's
    /// native token.
    #[clap(long, env)]
    pub native_price_estimators: NativePriceEstimators,

    /// Fallback native price estimators to use when all primary estimators
    /// are down.
    #[clap(long, env)]
    pub native_price_estimators_fallback: Option<NativePriceEstimators>,

    /// How many successful price estimates for each order will cause a fast
    /// or native price estimation to return its result early.
    /// The bigger the value the more the fast price estimation performs like
    /// the optimal price estimation.
    /// It's possible to pass values greater than the total number of enabled
    /// estimators but that will not have any further effect.
    #[clap(long, env, default_value = "2")]
    pub fast_price_estimation_results_required: NonZeroUsize,
}

impl std::fmt::Display for Arguments {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        let Arguments {
            shared,
            order_quoting,
            http_client,
            price_estimation,
            database_pool,
            config,
            bind_address,
            native_price_estimators,
            native_price_estimators_fallback,
            fast_price_estimation_results_required,
            db_write_url: db_url,
            db_read_url,
        } = self;

        write!(f, "{shared}")?;
        write!(f, "{order_quoting}")?;
        write!(f, "{http_client}")?;
        write!(f, "{price_estimation}")?;
        write!(f, "{database_pool}")?;
        writeln!(f, "config: {}", config.display())?;
        writeln!(f, "bind_address: {bind_address}")?;
        let _intentionally_ignored = db_url;
        writeln!(f, "db_url: SECRET")?;
        display_secret_option(f, "db_read_url", db_read_url.as_ref())?;
        writeln!(f, "native_price_estimators: {native_price_estimators}")?;
        writeln!(
            f,
            "native_price_estimators_fallback: {native_price_estimators_fallback:?}"
        )?;
        writeln!(
            f,
            "fast_price_estimation_results_required: {fast_price_estimation_results_required}"
        )?;

        Ok(())
    }
}
