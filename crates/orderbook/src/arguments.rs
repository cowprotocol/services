use {
    reqwest::Url,
    shared::{
        arguments::display_secret_option,
        http_client,
        price_estimation::{self},
    },
    std::{net::SocketAddr, path::PathBuf},
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

        Ok(())
    }
}
