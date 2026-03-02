use {
    crate::database::INSERT_BATCH_SIZE_DEFAULT,
    alloy::primitives::Address,
    anyhow::Context,
    shared::{
        arguments::{display_option, display_secret_option},
        http_client,
        price_estimation::{self},
    },
    std::{net::SocketAddr, num::NonZeroUsize, path::PathBuf, str::FromStr, time::Duration},
    url::Url,
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

    #[clap(flatten)]
    pub database_pool: shared::arguments::DatabasePoolConfig,

    /// Address of the ethflow contracts. If not specified, eth-flow orders are
    /// disabled.
    /// In general, one contract is sufficient for the service to function.
    /// Support for multiple contract was added to support transition period for
    /// integrators when the migration of the eth-flow contract happens.
    #[clap(long, env, use_value_delimiter = true)]
    pub ethflow_contracts: Vec<Address>,

    /// Timestamp at which we should start indexing eth-flow contract events.
    /// If there are already events in the database for a date later than this,
    /// then this date is ignored and can be omitted.
    #[clap(long, env)]
    pub ethflow_indexing_start: Option<u64>,

    #[clap(long, env, default_value = "0.0.0.0:9589")]
    pub metrics_address: SocketAddr,

    /// Address to bind the HTTP API server
    #[clap(long, env, default_value = "0.0.0.0:12088")]
    pub api_address: SocketAddr,

    /// Url of the Postgres database. By default connects to locally running
    /// postgres.
    #[clap(long, env, default_value = "postgresql://")]
    pub db_write_url: Url,

    /// The number of order events to insert in a single batch.
    #[clap(long, env, default_value_t = INSERT_BATCH_SIZE_DEFAULT)]
    pub insert_batch_size: NonZeroUsize,

    /// Skip syncing past events (useful for local deployments)
    #[clap(long, env, action = clap::ArgAction::Set, default_value = "false")]
    pub skip_event_sync: bool,

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

    /// Run the autopilot in a shadow mode by specifying an upstream CoW
    /// protocol deployment to pull auctions from. This will cause the autopilot
    /// to start a run loop where it performs solver competition on driver,
    /// and report and log the winner **without** requesting that any driver
    /// actually executes any settlements. Note that many of the `autopilot`'s
    /// typical features will be disabled in this mode, making many options
    /// ignored. This assumes co-location is enabled and does not require it
    /// being specified separately.
    #[clap(long, env)]
    pub shadow: Option<Url>,

    /// Time solvers have to compute a score per auction.
    #[clap(
        long,
        env,
        default_value = "15s",
        value_parser = humantime::parse_duration,
    )]
    pub solve_deadline: Duration,

    /// Configurations for indexing CoW AMMs. Supplied in the form of:
    /// "<factory1>|<helper1>|<block1>,<factory2>|<helper2>,<block2>"
    /// - factory is contract address emmiting CoW AMM deployment events.
    /// - helper is a contract address to interface with pools deployed by the
    ///   factory
    /// - block is the block at which indexing should start (should be 1 block
    ///   before the deployment of the factory)
    #[clap(long, env, use_value_delimiter = true)]
    pub cow_amm_configs: Vec<CowAmmConfig>,

    /// If a new run loop would start more than this amount of time after the
    /// system noticed the latest block, wait for the next block to appear
    /// before continuing the run loop.
    #[clap(long, env, default_value = "2s", value_parser = humantime::parse_duration)]
    pub max_run_loop_delay: Duration,

    /// Maximum timeout for fetching the native prices in the run loop
    /// If the value is 0, the native prices are fetched from the cache
    #[clap(long, env, default_value = "0s", value_parser = humantime::parse_duration)]
    pub run_loop_native_price_timeout: Duration,

    #[clap(long, env, default_value = "20")]
    /// The maximum number of winners per auction. Each winner will be allowed
    /// to settle their winning orders at the same time.
    pub max_winners_per_auction: NonZeroUsize,

    #[clap(long, env, default_value = "3")]
    /// The maximum allowed number of solutions to be proposed from a single
    /// solver, per auction.
    pub max_solutions_per_solver: NonZeroUsize,

    /// Archive node URL used to index CoW AMM
    #[clap(long, env)]
    pub archive_node_url: Option<Url>,

    /// Configures whether the autopilot filters out orders with insufficient
    /// balances.
    #[clap(long, env, default_value = "false", action = clap::ArgAction::Set)]
    pub disable_order_balance_filter: bool,

    /// Enables the usage of leader lock in the database
    /// The second instance of autopilot will act as a follower
    /// and not cut any auctions.
    #[clap(long, env, default_value = "false", action = clap::ArgAction::Set)]
    pub enable_leader_lock: bool,

    /// Enables brotli compression of `/solve` request bodies sent to drivers.
    #[clap(long, env, default_value = "false", action = clap::ArgAction::Set)]
    pub compress_solve_request: bool,

    /// Limits the amount of time the autopilot may spend running the
    /// maintenance logic between 2 auctions. When this times out we prefer
    /// running a not fully updated auction over stalling the protocol any
    /// further.
    #[clap(long, env, default_value = "5s", value_parser = humantime::parse_duration)]
    pub max_maintenance_timeout: Duration,
}

impl std::fmt::Display for CliArguments {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let Self {
            config,
            shared,
            order_quoting,
            http_client,
            price_estimation,
            database_pool,
            ethflow_contracts,
            ethflow_indexing_start,
            metrics_address,
            api_address,
            skip_event_sync,
            unsupported_tokens,
            min_order_validity_period,
            max_auction_age,
            submission_deadline,
            shadow,
            solve_deadline,
            db_write_url,
            insert_batch_size,
            max_settlement_transaction_wait,
            cow_amm_configs,
            max_run_loop_delay,
            run_loop_native_price_timeout,
            max_winners_per_auction,
            archive_node_url,
            max_solutions_per_solver,
            disable_order_balance_filter,
            enable_leader_lock,
            compress_solve_request,
            max_maintenance_timeout,
        } = self;
        write!(f, "{}", config.display())?;
        write!(f, "{shared}")?;
        write!(f, "{order_quoting}")?;
        write!(f, "{http_client}")?;
        write!(f, "{price_estimation}")?;
        write!(f, "{database_pool}")?;
        writeln!(f, "ethflow_contracts: {ethflow_contracts:?}")?;
        writeln!(f, "ethflow_indexing_start: {ethflow_indexing_start:?}")?;
        writeln!(f, "metrics_address: {metrics_address}")?;
        writeln!(f, "api_address: {api_address}")?;
        display_secret_option(f, "db_write_url", Some(&db_write_url))?;
        writeln!(f, "skip_event_sync: {skip_event_sync}")?;
        writeln!(f, "unsupported_tokens: {unsupported_tokens:?}")?;
        writeln!(
            f,
            "min_order_validity_period: {min_order_validity_period:?}"
        )?;
        writeln!(f, "max_auction_age: {max_auction_age:?}")?;
        writeln!(f, "submission_deadline: {submission_deadline}")?;
        display_option(f, "shadow", shadow)?;
        writeln!(f, "solve_deadline: {solve_deadline:?}")?;
        writeln!(f, "insert_batch_size: {insert_batch_size}")?;
        writeln!(
            f,
            "max_settlement_transaction_wait: {max_settlement_transaction_wait:?}"
        )?;
        writeln!(f, "cow_amm_configs: {cow_amm_configs:?}")?;
        writeln!(f, "max_run_loop_delay: {max_run_loop_delay:?}")?;
        writeln!(
            f,
            "run_loop_native_price_timeout: {run_loop_native_price_timeout:?}"
        )?;
        writeln!(f, "max_winners_per_auction: {max_winners_per_auction:?}")?;
        writeln!(f, "archive_node_url: {archive_node_url:?}")?;
        writeln!(f, "max_solutions_per_solver: {max_solutions_per_solver:?}")?;
        writeln!(
            f,
            "disable_order_balance_filter: {disable_order_balance_filter}"
        )?;
        writeln!(f, "enable_leader_lock: {enable_leader_lock}")?;
        writeln!(f, "compress_solve_request: {compress_solve_request}")?;
        writeln!(f, "max_maintenance_timeout: {max_maintenance_timeout:?}")?;
        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct CowAmmConfig {
    /// Which contract to index for CoW AMM deployment events.
    pub factory: Address,
    /// Which helper contract to use for interfacing with the indexed CoW AMMs.
    pub helper: Address,
    /// At which block indexing should start on the factory.
    pub index_start: u64,
}

impl FromStr for CowAmmConfig {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut parts = s.split('|');
        let factory = parts
            .next()
            .context("config is missing factory")?
            .parse()
            .context("could not parse factory as Address")?;
        let helper = parts
            .next()
            .context("config is missing helper")?
            .parse()
            .context("could not parse helper as Address")?;
        let index_start = parts
            .next()
            .context("config is missing index_start")?
            .parse()
            .context("could not parse index_start as u64")?;
        anyhow::ensure!(
            parts.next().is_none(),
            "supplied too many arguments for cow amm config"
        );

        Ok(Self {
            factory,
            helper,
            index_start,
        })
    }
}
