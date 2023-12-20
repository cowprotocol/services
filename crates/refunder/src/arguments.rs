use {
    clap::Parser,
    ethcontract::H160,
    shared::{arguments::display_option, ethrpc, http_client, logging_args_with_default_filter},
    std::time::Duration,
    tracing::level_filters::LevelFilter,
    url::Url,
};

logging_args_with_default_filter!(LoggingArguments, "warn,refunder=debug,shared=debug");

#[derive(Parser)]
pub struct Arguments {
    #[clap(flatten)]
    pub http_client: http_client::Arguments,

    #[clap(flatten)]
    pub ethrpc: ethrpc::Arguments,

    #[clap(flatten)]
    pub logging: LoggingArguments,

    /// Minimum time in seconds an order must have been valid for
    /// to be eligible for refunding
    #[clap(
        long,
        env,
        default_value = "2m",
        value_parser = humantime::parse_duration,
    )]
    pub min_validity_duration: Duration,

    /// Minimum slippage an order must have, in order
    /// to be eligble for refunding
    /// Front-end will place orders with a default slippage of 2%
    /// hence, we are requiring as a default 190 bps or 1.9 %
    #[clap(long, env, default_value = "190")]
    pub min_slippage_bps: u64,

    /// Url of the Postgres database. By default connects to locally running
    /// postgres.
    #[clap(long, env, default_value = "postgresql://")]
    pub db_url: Url,

    /// The Ethereum node URL to connect to.
    #[clap(long, env, default_value = "http://localhost:8545")]
    pub node_url: Url,

    /// The expected chain ID that the services are expected to run against.
    /// This can be optionally specified in order to check at startup whether
    /// the connected nodes match to detect misconfigurations.
    #[clap(long, env)]
    pub chain_id: Option<u64>,

    /// Address of the ethflow contract
    #[clap(long, env)]
    pub ethflow_contract: H160,

    #[clap(long, env, hide_env_values = true)]
    pub refunder_pk: String,

    /// The port at which we serve our metrics
    #[clap(long, env, default_value = "9590")]
    pub metrics_port: u16,
}

impl std::fmt::Display for Arguments {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.http_client)?;
        write!(f, "{}", self.ethrpc)?;
        writeln!(f, "min_validity_duration: {:?}", self.min_validity_duration)?;
        writeln!(f, "min_slippage_bps: {}", self.min_slippage_bps)?;
        writeln!(f, "db_url: SECRET")?;
        writeln!(f, "node_url: {}", self.node_url)?;
        display_option(f, "chain_id", &self.chain_id)?;
        writeln!(f, "ethflow_contract: {:?}", self.ethflow_contract)?;
        writeln!(f, "refunder_pk: SECRET")?;
        writeln!(f, "metrics_port: {}", self.metrics_port)?;
        Ok(())
    }
}
