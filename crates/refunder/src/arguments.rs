use {
    alloy::primitives::Address,
    clap::Parser,
    shared::{arguments::display_option, http_client, logging_args_with_default_filter, web3},
    std::time::Duration,
    url::Url,
};

logging_args_with_default_filter!(LoggingArguments, "warn,refunder=debug,shared=debug");

#[derive(Parser)]
pub struct Arguments {
    #[clap(flatten)]
    pub http_client: http_client::Arguments,

    #[clap(flatten)]
    pub ethrpc: web3::Arguments,

    #[clap(flatten)]
    pub logging: LoggingArguments,

    #[clap(flatten)]
    pub database_pool: shared::arguments::DatabasePoolConfig,

    /// Minimum time in seconds an order must have been valid for
    /// to be eligible for refunding
    #[clap(
        long,
        env,
        default_value = "2m",
        value_parser = humantime::parse_duration,
    )]
    pub min_validity_duration: Duration,

    /// Minimum *required* price deviation from quote (in basis points),
    /// for an order to be eligible for refunding.
    /// Negative values mean the order was placed with a better-than-quote price
    /// (less executable). For example:
    ///   - A value of `-10` allows refunding orders up to 0.10% better than
    ///     quote(price improvement)
    ///   - A value of `0` requires the order to be at least equal to quote
    ///   - A value of `190` (default) allows refunding only orders with ≥1.9%
    ///     slippage
    #[clap(long, env, default_value = "190")]
    pub min_price_deviation_bps: i64,

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

    /// Addresses of the ethflow contracts
    #[clap(long, env, use_value_delimiter = true)]
    pub ethflow_contracts: Vec<Address>,

    #[clap(long, env, hide_env_values = true)]
    pub refunder_pk: String,

    /// The port at which we serve our metrics
    #[clap(long, env, default_value = "9590")]
    pub metrics_port: u16,

    /// Maximum gas price (in wei) for submitting refund transactions
    /// Default is 2000 Gwei (2_000_000_000_000 wei)
    #[clap(long, env, default_value = "2000000000000")]
    pub max_gas_price: u64,

    /// Starting priority fee tip (in wei) for refund transactions
    /// Default is 30 Gwei (30_000_000_000 wei)
    #[clap(long, env, default_value = "30000000000")]
    pub start_priority_fee_tip: u64,

    /// Time period in which the service looks for refundable orders.
    #[clap(long, env, value_parser = humantime::parse_duration, default_value = "1 week")]
    pub lookback_time: Duration,
}

impl std::fmt::Display for Arguments {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let Arguments {
            http_client,
            ethrpc,
            min_validity_duration,
            min_price_deviation_bps,
            node_url,
            chain_id,
            ethflow_contracts,
            metrics_port,
            logging,
            database_pool,
            db_url,
            refunder_pk,
            max_gas_price,
            start_priority_fee_tip,
            lookback_time,
        } = self;

        write!(f, "{http_client}")?;
        write!(f, "{ethrpc}")?;
        write!(f, "{logging}")?;
        write!(f, "{database_pool}")?;
        writeln!(f, "min_validity_duration: {min_validity_duration:?}")?;
        writeln!(f, "min_price_deviation_bps: {min_price_deviation_bps}")?;
        let _intentionally_ignored = db_url;
        writeln!(f, "db_url: SECRET")?;
        writeln!(f, "node_url: {node_url}")?;
        display_option(f, "chain_id", chain_id)?;
        writeln!(f, "ethflow_contracts: {ethflow_contracts:?}")?;
        let _intentionally_ignored = refunder_pk;
        writeln!(f, "refunder_pk: SECRET")?;
        writeln!(f, "metrics_port: {metrics_port}")?;
        writeln!(f, "max_gas_price: {max_gas_price}")?;
        writeln!(f, "start_priority_fee_tip: {start_priority_fee_tip}")?;
        writeln!(f, "lookback_time: {lookback_time:?}")?;
        Ok(())
    }
}
