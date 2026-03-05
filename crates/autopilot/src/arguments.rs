use {
    alloy::primitives::Address,
    shared::http_client,
    std::{path::PathBuf, time::Duration},
};

#[derive(clap::Parser)]
pub struct CliArguments {
    #[clap(long, env)]
    pub config: PathBuf,

    #[clap(flatten)]
    pub shared: shared::arguments::Arguments,

    #[clap(flatten)]
    pub order_quoting: shared::arguments::OrderQuotingArguments,

    #[clap(flatten)]
    pub http_client: http_client::Arguments,

    #[clap(flatten)]
    pub price_estimation: price_estimation::Arguments,

    /// List of token addresses to be ignored throughout service
    #[clap(long, env, use_value_delimiter = true)]
    pub unsupported_tokens: Vec<Address>,

    /// The minimum amount of time an order has to be valid for.
    #[clap(
        long,
        env,
        default_value = "1m",
        value_parser = humantime::parse_duration,
    )]
    pub min_order_validity_period: Duration,

    /// If the auction hasn't been updated in this amount of time the pod fails
    /// the liveness check. Expects a value in seconds.
    #[clap(
        long,
        env,
        default_value = "5m",
        value_parser = humantime::parse_duration,
    )]
    pub max_auction_age: Duration,

    /// The maximum number of blocks to wait for a settlement to appear on
    /// chain.
    #[clap(long, env, default_value = "5")]
    pub submission_deadline: usize,

    /// The amount of time that the autopilot waits looking for a settlement
    /// transaction onchain after the driver acknowledges the receipt of a
    /// settlement.
    #[clap(
        long,
        env,
        default_value = "1m",
        value_parser = humantime::parse_duration,
    )]
    pub max_settlement_transaction_wait: Duration,

    /// Time solvers have to compute a score per auction.
    #[clap(
        long,
        env,
        default_value = "15s",
        value_parser = humantime::parse_duration,
    )]
    pub solve_deadline: Duration,
}

impl std::fmt::Display for CliArguments {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let Self {
            config,
            shared,
            order_quoting,
            http_client,
            price_estimation,
            unsupported_tokens,
            min_order_validity_period,
            max_auction_age,
            submission_deadline,
            solve_deadline,
            max_settlement_transaction_wait,
        } = self;
        write!(f, "{}", config.display())?;
        write!(f, "{shared}")?;
        write!(f, "{order_quoting}")?;
        write!(f, "{http_client}")?;
        write!(f, "{price_estimation}")?;
        writeln!(f, "unsupported_tokens: {unsupported_tokens:?}")?;
        writeln!(
            f,
            "min_order_validity_period: {min_order_validity_period:?}"
        )?;
        writeln!(f, "max_auction_age: {max_auction_age:?}")?;
        writeln!(f, "submission_deadline: {submission_deadline}")?;
        writeln!(f, "solve_deadline: {solve_deadline:?}")?;
        writeln!(
            f,
            "max_settlement_transaction_wait: {max_settlement_transaction_wait:?}"
        )?;
        Ok(())
    }
}
