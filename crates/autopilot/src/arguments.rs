use {
    crate::{domain::fee::FeeFactor, infra},
    anyhow::{Context, anyhow, ensure},
    clap::ValueEnum,
    primitive_types::{H160, U256},
    shared::{
        arguments::{display_list, display_option},
        bad_token::token_owner_finder,
        http_client,
        price_estimation::{self, NativePriceEstimators},
    },
    std::{
        fmt,
        fmt::{Display, Formatter},
        net::SocketAddr,
        num::NonZeroUsize,
        str::FromStr,
        time::Duration,
    },
    url::Url,
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

    /// Address of the ethflow contracts. If not specified, eth-flow orders are
    /// disabled.
    /// In general, one contract is sufficient for the service to function.
    /// Support for multiple contract was added to support transition period for
    /// integrators when the migration of the eth-flow contract happens.
    #[clap(long, env, use_value_delimiter = true)]
    pub ethflow_contracts: Vec<H160>,

    /// Timestamp at which we should start indexing eth-flow contract events.
    /// If there are already events in the database for a date later than this,
    /// then this date is ignored and can be omitted.
    #[clap(long, env)]
    pub ethflow_indexing_start: Option<u64>,

    /// A tracing Ethereum node URL to connect to, allowing a separate node URL
    /// to be used exclusively for tracing calls.
    #[clap(long, env)]
    pub tracing_node_url: Option<Url>,

    #[clap(long, env, default_value = "0.0.0.0:9589")]
    pub metrics_address: SocketAddr,

    /// Url of the Postgres database. By default connects to locally running
    /// postgres.
    #[clap(long, env, default_value = "postgresql://")]
    pub db_url: Url,

    /// The number of order events to insert in a single batch.
    #[clap(long, env, default_value = "500")]
    pub insert_batch_size: NonZeroUsize,

    /// Skip syncing past events (useful for local deployments)
    #[clap(long, env, action = clap::ArgAction::Set, default_value = "false")]
    pub skip_event_sync: bool,

    /// List of token addresses that should be allowed regardless of whether the
    /// bad token detector thinks they are bad. Base tokens are
    /// automatically allowed.
    #[clap(long, env, use_value_delimiter = true)]
    pub allowed_tokens: Vec<H160>,

    /// List of token addresses to be ignored throughout service
    #[clap(long, env, use_value_delimiter = true)]
    pub unsupported_tokens: Vec<H160>,

    /// The number of pairs that are automatically updated in the pool cache.
    #[clap(long, env, default_value = "200")]
    pub pool_cache_lru_size: NonZeroUsize,

    /// Which estimators to use to estimate token prices in terms of the chain's
    /// native token. Estimators with the same name need to also be specified as
    /// built-in, legacy or external price estimators (lookup happens in this
    /// order in case of name collisions)
    #[clap(long, env)]
    pub native_price_estimators: NativePriceEstimators,

    /// How many successful price estimates for each order will cause a native
    /// price estimation to return its result early. It's possible to pass
    /// values greater than the total number of enabled estimators but that
    /// will not have any further effect.
    #[clap(long, env, default_value = "2")]
    pub native_price_estimation_results_required: NonZeroUsize,

    /// The minimum amount of time an order has to be valid for.
    #[clap(
        long,
        env,
        default_value = "1m",
        value_parser = humantime::parse_duration,
    )]
    pub min_order_validity_period: Duration,

    /// List of account addresses to be denied from order creation
    #[clap(long, env, use_value_delimiter = true)]
    pub banned_users: Vec<H160>,

    /// If the auction hasn't been updated in this amount of time the pod fails
    /// the liveness check. Expects a value in seconds.
    #[clap(
        long,
        env,
        default_value = "5m",
        value_parser = humantime::parse_duration,
    )]
    pub max_auction_age: Duration,

    /// Used to filter out limit orders with prices that are too far from the
    /// market price. 0 means no filtering.
    #[clap(long, env, default_value = "0")]
    pub limit_order_price_factor: f64,

    /// The URL of a list of tokens our settlement contract is willing to
    /// internalize.
    #[clap(long, env)]
    pub trusted_tokens_url: Option<Url>,

    /// Hardcoded list of trusted tokens to use in addition to
    /// `trusted_tokens_url`.
    #[clap(long, env, use_value_delimiter = true)]
    pub trusted_tokens: Option<Vec<H160>>,

    /// Time interval after which the trusted tokens list needs to be updated.
    #[clap(
        long,
        env,
        default_value = "1h",
        value_parser = humantime::parse_duration,
    )]
    pub trusted_tokens_update_interval: Duration,

    /// A list of drivers in the following format:
    /// `<NAME>|<URL>|<SUBMISSION_ADDRESS>|<FAIRNESS_THRESHOLD>`
    #[clap(long, env, use_value_delimiter = true)]
    pub drivers: Vec<Solver>,

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

    /// Describes how the protocol fees should be calculated.
    #[clap(long, env, use_value_delimiter = true)]
    pub fee_policies: Vec<FeePolicy>,

    /// Maximum partner fee allow. If the partner fee specified is greater than
    /// this maximum, the partner fee will be capped
    #[clap(long, env, default_value = "0.01")]
    pub fee_policy_max_partner_fee: FeeFactor,

    /// Arguments for uploading information to S3.
    #[clap(flatten)]
    pub s3: infra::persistence::cli::S3,

    /// Time interval in days between each cleanup operation of the
    /// `order_events` database table.
    #[clap(long, env, default_value = "1d", value_parser = humantime::parse_duration)]
    pub order_events_cleanup_interval: Duration,

    /// Age threshold in days for order events to be eligible for cleanup in the
    /// `order_events` database table.
    #[clap(long, env, default_value = "30d", value_parser = humantime::parse_duration)]
    pub order_events_cleanup_threshold: Duration,

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

    #[clap(long, env, default_value = "1")]
    /// The maximum number of winners per auction. Each winner will be allowed
    /// to settle their winning orders at the same time.
    pub max_winners_per_auction: usize,

    #[clap(long, env, default_value = "3")]
    /// The maximum allowed number of solutions to be proposed from a single
    /// solver, per auction.
    pub max_solutions_per_solver: usize,

    /// Archive node URL used to index CoW AMM
    #[clap(long, env)]
    pub archive_node_url: Option<Url>,

    /// Configuration for the solver participation guard.
    #[clap(flatten)]
    pub db_based_solver_participation_guard: DbBasedSolverParticipationGuardConfig,
}

#[derive(Debug, clap::Parser)]
pub struct DbBasedSolverParticipationGuardConfig {
    /// Enables or disables the solver participation guard
    #[clap(
        id = "db_enabled",
        long = "db-based-solver-participation-guard-enabled",
        env = "DB_BASED_SOLVER_PARTICIPATION_GUARD_ENABLED",
        default_value = "true"
    )]
    pub enabled: bool,

    /// Sets the duration for which the solver remains blacklisted.
    /// Technically, the time-to-live for the solver participation blacklist
    /// cache.
    #[clap(long, env, default_value = "5m", value_parser = humantime::parse_duration)]
    pub solver_blacklist_cache_ttl: Duration,

    #[clap(flatten)]
    pub non_settling_solvers_finder_config: NonSettlingSolversFinderConfig,

    #[clap(flatten)]
    pub low_settling_solvers_finder_config: LowSettlingSolversFinderConfig,
}

#[derive(Debug, clap::Parser)]
pub struct NonSettlingSolversFinderConfig {
    /// Enables search of non-settling solvers.
    #[clap(
        id = "non_settling_solvers_blacklisting_enabled",
        long = "non-settling-solvers-blacklisting-enabled",
        env = "NON_SETTLING_SOLVERS_BLACKLISTING_ENABLED",
        default_value = "true"
    )]
    pub enabled: bool,

    /// The number of last auctions to check solver participation eligibility.
    #[clap(
        id = "non_settling_last_auctions_participation_count",
        long = "non-settling-last-auctions-participation-count",
        env = "NON_SETTLING_LAST_AUCTIONS_PARTICIPATION_COUNT",
        default_value = "3"
    )]
    pub last_auctions_participation_count: u32,
}

#[derive(Debug, clap::Parser)]
pub struct LowSettlingSolversFinderConfig {
    /// Enables search of non-settling solvers.
    #[clap(
        id = "low_settling_solvers_blacklisting_enabled",
        long = "low-settling-solvers-blacklisting-enabled",
        env = "LOW_SETTLING_SOLVERS_BLACKLISTING_ENABLED",
        default_value = "true"
    )]
    pub enabled: bool,

    /// The number of last auctions to check solver participation eligibility.
    #[clap(
        id = "low_settling_last_auctions_participation_count",
        long = "low-settling-last-auctions-participation-count",
        env = "LOW_SETTLING_LAST_AUCTIONS_PARTICIPATION_COUNT",
        default_value = "100"
    )]
    pub last_auctions_participation_count: u32,

    /// A max failure rate for a solver to remain eligible for
    /// participation in the competition. Otherwise, the solver will be
    /// banned.
    #[clap(long, env, default_value = "0.9")]
    pub solver_max_settlement_failure_rate: f64,
}

impl std::fmt::Display for Arguments {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let Self {
            shared,
            order_quoting,
            http_client,
            token_owner_finder,
            price_estimation,
            tracing_node_url,
            ethflow_contracts,
            ethflow_indexing_start,
            metrics_address,
            skip_event_sync,
            allowed_tokens,
            unsupported_tokens,
            pool_cache_lru_size,
            native_price_estimators,
            min_order_validity_period,
            banned_users,
            max_auction_age,
            limit_order_price_factor,
            trusted_tokens_url,
            trusted_tokens,
            trusted_tokens_update_interval,
            drivers,
            submission_deadline,
            shadow,
            solve_deadline,
            fee_policies,
            fee_policy_max_partner_fee,
            order_events_cleanup_interval,
            order_events_cleanup_threshold,
            db_url,
            insert_batch_size,
            native_price_estimation_results_required,
            max_settlement_transaction_wait,
            s3,
            cow_amm_configs,
            max_run_loop_delay,
            run_loop_native_price_timeout,
            max_winners_per_auction,
            archive_node_url,
            max_solutions_per_solver,
            db_based_solver_participation_guard,
        } = self;

        write!(f, "{}", shared)?;
        write!(f, "{}", order_quoting)?;
        write!(f, "{}", http_client)?;
        write!(f, "{}", token_owner_finder)?;
        write!(f, "{}", price_estimation)?;
        display_option(f, "tracing_node_url", tracing_node_url)?;
        writeln!(f, "ethflow_contracts: {:?}", ethflow_contracts)?;
        writeln!(f, "ethflow_indexing_start: {:?}", ethflow_indexing_start)?;
        writeln!(f, "metrics_address: {}", metrics_address)?;
        let _intentionally_ignored = db_url;
        writeln!(f, "db_url: SECRET")?;
        writeln!(f, "skip_event_sync: {}", skip_event_sync)?;
        writeln!(f, "allowed_tokens: {:?}", allowed_tokens)?;
        writeln!(f, "unsupported_tokens: {:?}", unsupported_tokens)?;
        writeln!(f, "pool_cache_lru_size: {}", pool_cache_lru_size)?;
        writeln!(f, "native_price_estimators: {}", native_price_estimators)?;
        writeln!(
            f,
            "min_order_validity_period: {:?}",
            min_order_validity_period
        )?;
        writeln!(f, "banned_users: {:?}", banned_users)?;
        writeln!(f, "max_auction_age: {:?}", max_auction_age)?;
        writeln!(
            f,
            "limit_order_price_factor: {:?}",
            limit_order_price_factor
        )?;
        display_option(f, "trusted_tokens_url", trusted_tokens_url)?;
        writeln!(f, "trusted_tokens: {:?}", trusted_tokens)?;
        writeln!(
            f,
            "trusted_tokens_update_interval: {:?}",
            trusted_tokens_update_interval
        )?;
        display_list(f, "drivers", drivers.iter())?;
        writeln!(f, "submission_deadline: {}", submission_deadline)?;
        display_option(f, "shadow", shadow)?;
        writeln!(f, "solve_deadline: {:?}", solve_deadline)?;
        writeln!(f, "fee_policies: {:?}", fee_policies)?;
        writeln!(
            f,
            "fee_policy_max_partner_fee: {:?}",
            fee_policy_max_partner_fee
        )?;
        writeln!(
            f,
            "order_events_cleanup_interval: {:?}",
            order_events_cleanup_interval
        )?;
        writeln!(
            f,
            "order_events_cleanup_threshold: {:?}",
            order_events_cleanup_threshold
        )?;
        writeln!(f, "insert_batch_size: {}", insert_batch_size)?;
        writeln!(
            f,
            "native_price_estimation_results_required: {}",
            native_price_estimation_results_required
        )?;
        writeln!(
            f,
            "max_settlement_transaction_wait: {:?}",
            max_settlement_transaction_wait
        )?;
        writeln!(f, "s3: {:?}", s3)?;
        writeln!(f, "cow_amm_configs: {:?}", cow_amm_configs)?;
        writeln!(f, "max_run_loop_delay: {:?}", max_run_loop_delay)?;
        writeln!(
            f,
            "run_loop_native_price_timeout: {:?}",
            run_loop_native_price_timeout
        )?;
        writeln!(f, "max_winners_per_auction: {:?}", max_winners_per_auction)?;
        writeln!(f, "archive_node_url: {:?}", archive_node_url)?;
        writeln!(
            f,
            "max_solutions_per_solver: {:?}",
            max_solutions_per_solver
        )?;
        writeln!(
            f,
            "db_based_solver_participation_guard: {:?}",
            db_based_solver_participation_guard
        )?;
        Ok(())
    }
}

/// External solver driver configuration
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Solver {
    pub name: String,
    pub url: Url,
    pub submission_account: Account,
    pub fairness_threshold: Option<U256>,
    pub requested_timeout_on_problems: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Account {
    /// AWS KMS is used to retrieve the solver public key
    Kms(Arn),
    /// Solver public key
    Address(H160),
}

// Wrapper type for AWS ARN identifiers
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Arn(pub String);

impl FromStr for Arn {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        // Could be more strict here, but this should suffice to catch unintended
        // configuration mistakes
        if s.starts_with("arn:aws:kms:") {
            Ok(Self(s.to_string()))
        } else {
            Err(anyhow!("Invalid ARN identifier: {}", s))
        }
    }
}

impl Display for Solver {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "{}({})", self.name, self.url)
    }
}

impl FromStr for Solver {
    type Err = anyhow::Error;

    fn from_str(solver: &str) -> anyhow::Result<Self> {
        let parts: Vec<&str> = solver.split('|').collect();
        ensure!(parts.len() >= 3, "not enough arguments for external solver");
        let (name, url) = (parts[0], parts[1]);
        let url: Url = url.parse()?;
        let submission_account = match Arn::from_str(parts[2]) {
            Ok(value) => Account::Kms(value),
            _ => Account::Address(H160::from_str(parts[2]).context("failed to parse submission")?),
        };

        let mut fairness_threshold: Option<U256> = Default::default();
        let mut requested_timeout_on_problems = false;

        if let Some(value) = parts.get(3) {
            match U256::from_dec_str(value) {
                Ok(parsed_fairness_threshold) => {
                    fairness_threshold = Some(parsed_fairness_threshold);
                }
                Err(_) => {
                    requested_timeout_on_problems =
                        value.to_lowercase() == "requested-timeout-on-problems";
                }
            }
        };

        if let Some(value) = parts.get(4) {
            requested_timeout_on_problems = value.to_lowercase() == "requested-timeout-on-problems";
        }

        Ok(Self {
            name: name.to_owned(),
            url,
            fairness_threshold,
            submission_account,
            requested_timeout_on_problems,
        })
    }
}

/// A fee policy to be used for orders base on it's class.
/// Examples:
/// - Surplus with a high enough cap for limit orders: surplus:0.5:0.9:limit
///
/// - Surplus with cap for market orders: surplus:0.5:0.06:market
///
/// - Price improvement with a high enough cap for any order class:
///   price_improvement:0.5:0.9:any
///
/// - Price improvement with cap for limit orders:
///   price_improvement:0.5:0.06:limit
///
/// - Volume based fee for any order class: volume:0.1:any
#[derive(Debug, Clone)]
pub struct FeePolicy {
    pub fee_policy_kind: FeePolicyKind,
    pub fee_policy_order_class: FeePolicyOrderClass,
}

#[derive(clap::Parser, Debug, Clone)]
pub enum FeePolicyKind {
    /// How much of the order's surplus should be taken as a protocol fee.
    Surplus {
        factor: FeeFactor,
        max_volume_factor: FeeFactor,
    },
    /// How much of the order's price improvement should be taken as a protocol
    /// fee where price improvement is a difference between the executed price
    /// and the best quote.
    PriceImprovement {
        factor: FeeFactor,
        max_volume_factor: FeeFactor,
    },
    /// How much of the order's volume should be taken as a protocol fee.
    Volume { factor: FeeFactor },
}

#[derive(clap::Parser, clap::ValueEnum, Clone, Debug)]
pub enum FeePolicyOrderClass {
    /// If a fee policy needs to be applied to in-market orders.
    Market,
    /// If a fee policy needs to be applied to limit orders.
    Limit,
    /// If a fee policy needs to be applied regardless of the order class.
    Any,
}

impl FromStr for FeePolicy {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut parts = s.split(':');
        let kind = parts.next().context("missing fee policy kind")?;
        let fee_policy_kind = match kind {
            "surplus" => {
                let factor = parts
                    .next()
                    .context("missing surplus factor")?
                    .parse::<f64>()
                    .map_err(|e| anyhow::anyhow!("invalid surplus factor: {}", e))?;
                let max_volume_factor = parts
                    .next()
                    .context("missing max volume factor")?
                    .parse::<f64>()
                    .map_err(|e| anyhow::anyhow!("invalid max volume factor: {}", e))?;
                Ok(FeePolicyKind::Surplus {
                    factor: factor.try_into()?,
                    max_volume_factor: max_volume_factor.try_into()?,
                })
            }
            "priceImprovement" => {
                let factor = parts
                    .next()
                    .context("missing price improvement factor")?
                    .parse::<f64>()
                    .map_err(|e| anyhow::anyhow!("invalid price improvement factor: {}", e))?;
                let max_volume_factor = parts
                    .next()
                    .context("missing price improvement max volume factor")?
                    .parse::<f64>()
                    .map_err(|e| {
                        anyhow::anyhow!("invalid price improvement max volume factor: {}", e)
                    })?;
                Ok(FeePolicyKind::PriceImprovement {
                    factor: factor.try_into()?,
                    max_volume_factor: max_volume_factor.try_into()?,
                })
            }
            "volume" => {
                let factor = parts
                    .next()
                    .context("missing volume factor")?
                    .parse::<f64>()
                    .map_err(|e| anyhow::anyhow!("invalid volume factor: {}", e))?;
                Ok(FeePolicyKind::Volume {
                    factor: factor.try_into()?,
                })
            }
            _ => Err(anyhow::anyhow!("invalid fee policy kind: {}", kind)),
        }?;
        let fee_policy_order_class = FeePolicyOrderClass::from_str(
            parts.next().context("missing fee policy order class")?,
            true,
        )
        .map_err(|e| anyhow::anyhow!("invalid fee policy order class: {}", e))?;

        Ok(FeePolicy {
            fee_policy_kind,
            fee_policy_order_class,
        })
    }
}

#[derive(Debug, Clone)]
pub struct CowAmmConfig {
    /// Which contract to index for CoW AMM deployment events.
    pub factory: H160,
    /// Which helper contract to use for interfacing with the indexed CoW AMMs.
    pub helper: H160,
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
            .context("could not parse factory as H160")?;
        let helper = parts
            .next()
            .context("config is missing helper")?
            .parse()
            .context("could not parse helper as H160")?;
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

#[cfg(test)]
mod test {
    use {super::*, hex_literal::hex};

    #[test]
    fn test_fee_factor_limits() {
        let policies = vec![
            "volume:1.0:market",
            "volume:-1.0:limit",
            "surplus:1.0:0.5:any",
            "surplus:0.5:1.0:limit",
            "surplus:0.5:-1.0:market",
            "surplus:-1.0:0.5:limit",
            "priceImprovement:1.0:0.5:market",
            "priceImprovement:-1.0:0.5:any",
            "priceImprovement:0.5:1.0:market",
            "priceImprovement:0.5:-1.0:limit",
        ];

        for policy in policies {
            assert!(
                FeePolicy::from_str(policy)
                    .err()
                    .unwrap()
                    .to_string()
                    .contains("Factor must be in the range [0, 1)"),
            )
        }
    }

    #[test]
    fn parse_driver_submission_account_address() {
        let argument = "name1|http://localhost:8080|0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2";
        let driver = Solver::from_str(argument).unwrap();
        let expected = Solver {
            name: "name1".into(),
            url: Url::parse("http://localhost:8080").unwrap(),
            fairness_threshold: None,
            requested_timeout_on_problems: false,
            submission_account: Account::Address(H160::from_slice(&hex!(
                "C02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2"
            ))),
        };
        assert_eq!(driver, expected);
    }

    #[test]
    fn parse_driver_submission_account_arn() {
        let argument = "name1|http://localhost:8080|arn:aws:kms:supersecretstuff";
        let driver = Solver::from_str(argument).unwrap();
        let expected = Solver {
            name: "name1".into(),
            url: Url::parse("http://localhost:8080").unwrap(),
            fairness_threshold: None,
            requested_timeout_on_problems: false,
            submission_account: Account::Kms(
                Arn::from_str("arn:aws:kms:supersecretstuff").unwrap(),
            ),
        };
        assert_eq!(driver, expected);
    }

    #[test]
    fn parse_driver_with_threshold() {
        let argument = "name1|http://localhost:8080|0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2|1000000000000000000";
        let driver = Solver::from_str(argument).unwrap();
        let expected = Solver {
            name: "name1".into(),
            url: Url::parse("http://localhost:8080").unwrap(),
            submission_account: Account::Address(H160::from_slice(&hex!(
                "C02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2"
            ))),
            fairness_threshold: Some(U256::exp10(18)),
            requested_timeout_on_problems: false,
        };
        assert_eq!(driver, expected);
    }

    #[test]
    fn parse_driver_with_accepts_unsettled_blocking_flag() {
        let argument =
            "name1|http://localhost:8080|0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2|requested-timeout-on-problems";
        let driver = Solver::from_str(argument).unwrap();
        let expected = Solver {
            name: "name1".into(),
            url: Url::parse("http://localhost:8080").unwrap(),
            submission_account: Account::Address(H160::from_slice(&hex!(
                "C02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2"
            ))),
            fairness_threshold: None,
            requested_timeout_on_problems: true,
        };
        assert_eq!(driver, expected);
    }

    #[test]
    fn parse_driver_with_threshold_and_accepts_unsettled_blocking_flag() {
        let argument = "name1|http://localhost:8080|0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2|1000000000000000000|requested-timeout-on-problems";
        let driver = Solver::from_str(argument).unwrap();
        let expected = Solver {
            name: "name1".into(),
            url: Url::parse("http://localhost:8080").unwrap(),
            submission_account: Account::Address(H160::from_slice(&hex!(
                "C02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2"
            ))),
            fairness_threshold: Some(U256::exp10(18)),
            requested_timeout_on_problems: true,
        };
        assert_eq!(driver, expected);
    }
}
