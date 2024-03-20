use {
    crate::infra,
    primitive_types::{H160, U256},
    shared::{
        arguments::{display_list, display_option, ExternalSolver},
        bad_token::token_owner_finder,
        http_client,
        price_estimation::{self, NativePriceEstimators},
    },
    std::{net::SocketAddr, num::NonZeroUsize, str::FromStr, time::Duration},
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

    /// Address of the ethflow contract. If not specified, eth-flow orders are
    /// disabled.
    #[clap(long, env)]
    pub ethflow_contract: Option<H160>,

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

    /// The amount of time a classification of a token into good or
    /// bad is valid for.
    #[clap(
        long,
        env,
        default_value = "10m",
        value_parser = humantime::parse_duration,
    )]
    pub token_quality_cache_expiry: Duration,

    /// The number of pairs that are automatically updated in the pool cache.
    #[clap(long, env, default_value = "200")]
    pub pool_cache_lru_size: NonZeroUsize,

    /// Which estimators to use to estimate token prices in terms of the chain's
    /// native token. Estimators with the same name need to also be specified as
    /// built-in, legacy or external price estimators (lookup happens in this
    /// order in case of name collisions)
    #[clap(long, env, default_value_t)]
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

    /// The time between auction updates.
    #[clap(long, env, default_value = "10s", value_parser = humantime::parse_duration)]
    pub auction_update_interval: Duration,

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

    /// A list of drivers in the following format: `<NAME>|<URL>,<NAME>|<URL>`
    #[clap(long, env, use_value_delimiter = true)]
    pub drivers: Vec<ExternalSolver>,

    /// The maximum number of blocks to wait for a settlement to appear on
    /// chain.
    #[clap(long, env, default_value = "5")]
    pub submission_deadline: usize,

    /// The maximum number of blocks to wait for a settlement to appear on
    /// chain, in addition to the submission deadline. This is used to ensure
    /// that the settlement mined at the very end on the deadline and reorged,
    /// is still considered for payout.
    #[clap(long, env, default_value = "5")]
    pub additional_deadline_for_rewards: usize,

    /// Cap used for CIP20 score calculation. Defaults to 0.01 ETH.
    #[clap(long, env, default_value = "0.01", value_parser = shared::arguments::wei_from_ether)]
    pub score_cap: U256,

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

    /// Describes how the protocol fee should be calculated.
    #[clap(flatten)]
    pub fee_policy: FeePolicy,

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
            ethflow_contract,
            ethflow_indexing_start,
            metrics_address,
            skip_event_sync,
            allowed_tokens,
            unsupported_tokens,
            token_quality_cache_expiry,
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
            additional_deadline_for_rewards,
            score_cap,
            shadow,
            solve_deadline,
            fee_policy,
            order_events_cleanup_interval,
            order_events_cleanup_threshold,
            db_url,
            insert_batch_size,
            native_price_estimation_results_required,
            auction_update_interval,
            max_settlement_transaction_wait,
            s3,
        } = self;

        write!(f, "{}", shared)?;
        write!(f, "{}", order_quoting)?;
        write!(f, "{}", http_client)?;
        write!(f, "{}", token_owner_finder)?;
        write!(f, "{}", price_estimation)?;
        display_option(f, "tracing_node_url", tracing_node_url)?;
        writeln!(f, "ethflow_contract: {:?}", ethflow_contract)?;
        writeln!(f, "ethflow_indexing_start: {:?}", ethflow_indexing_start)?;
        writeln!(f, "metrics_address: {}", metrics_address)?;
        let _intentionally_ignored = db_url;
        writeln!(f, "db_url: SECRET")?;
        writeln!(f, "skip_event_sync: {}", skip_event_sync)?;
        writeln!(f, "allowed_tokens: {:?}", allowed_tokens)?;
        writeln!(f, "unsupported_tokens: {:?}", unsupported_tokens)?;
        writeln!(
            f,
            "token_quality_cache_expiry: {:?}",
            token_quality_cache_expiry
        )?;
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
        writeln!(
            f,
            "additional_deadline_for_rewards: {}",
            additional_deadline_for_rewards
        )?;
        writeln!(f, "score_cap: {}", score_cap)?;
        display_option(f, "shadow", shadow)?;
        writeln!(f, "solve_deadline: {:?}", solve_deadline)?;
        writeln!(f, "fee_policy: {:?}", fee_policy)?;
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
        writeln!(f, "auction_update_interval: {:?}", auction_update_interval)?;
        writeln!(
            f,
            "max_settlement_transaction_wait: {:?}",
            max_settlement_transaction_wait
        )?;
        writeln!(f, "s3: {:?}", s3)?;
        Ok(())
    }
}

#[derive(clap::Parser, Debug, Clone)]
pub struct FeePolicy {
    /// Type of fee policy to use. Examples:
    ///
    /// - Surplus without cap
    /// surplus:0.5:1.0
    ///
    /// - Surplus with cap:
    /// surplus:0.5:0.06
    ///
    /// - Price improvement without cap:
    /// price_improvement:0.5:1.0
    ///
    /// - Price improvement with cap:
    /// price_improvement:0.5:0.06
    ///
    /// - Volume based:
    /// volume:0.1
    #[clap(long, env, default_value = "surplus:0.0:1.0")]
    pub fee_policy_kind: FeePolicyKind,

    /// Should protocol fees be collected or skipped for orders whose
    /// limit price at order creation time suggests they can be immediately
    /// filled.
    #[clap(long, env, action = clap::ArgAction::Set, default_value = "true")]
    pub fee_policy_skip_market_orders: bool,
}

#[derive(clap::Parser, Debug, Clone)]
pub enum FeePolicyKind {
    /// How much of the order's surplus should be taken as a protocol fee.
    Surplus { factor: f64, max_volume_factor: f64 },
    /// How much of the order's price improvement should be taken as a protocol
    /// fee where price improvement is a difference between the executed price
    /// and the best quote.
    PriceImprovement { factor: f64, max_volume_factor: f64 },
    /// How much of the order's volume should be taken as a protocol fee.
    Volume { factor: f64 },
}

fn validate_factor(factor: f64) -> Result<(), String> {
    if !(0.0..1.0).contains(&factor) {
        return Err(format!("Factor must be in the range [0, 1), got {factor}",));
    }
    Ok(())
}

impl FromStr for FeePolicyKind {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut parts = s.split(':');
        let kind = parts.next().ok_or("missing fee policy kind")?;
        match kind {
            "surplus" => {
                let factor = parts
                    .next()
                    .ok_or("missing surplus factor")?
                    .parse::<f64>()
                    .map_err(|e| format!("invalid surplus factor: {}", e))?;
                validate_factor(factor)?;
                let max_volume_factor = parts
                    .next()
                    .ok_or("missing max volume factor")?
                    .parse::<f64>()
                    .map_err(|e| format!("invalid max volume factor: {}", e))?;
                Ok(Self::Surplus {
                    factor,
                    max_volume_factor,
                })
            }
            "priceImprovement" => {
                let factor = parts
                    .next()
                    .ok_or("missing price improvement factor")?
                    .parse::<f64>()
                    .map_err(|e| format!("invalid price improvement factor: {}", e))?;
                validate_factor(factor)?;
                let max_volume_factor = parts
                    .next()
                    .ok_or("missing price improvement max volume factor")?
                    .parse::<f64>()
                    .map_err(|e| format!("invalid price improvement max volume factor: {}", e))?;
                Ok(Self::PriceImprovement {
                    factor,
                    max_volume_factor,
                })
            }
            "volume" => {
                let factor = parts
                    .next()
                    .ok_or("missing volume factor")?
                    .parse::<f64>()
                    .map_err(|e| format!("invalid volume factor: {}", e))?;
                validate_factor(factor)?;
                Ok(Self::Volume { factor })
            }
            _ => Err(format!("invalid fee policy kind: {}", kind)),
        }
    }
}

#[cfg(test)]
mod test {
    use {super::*, rstest::rstest};

    #[rstest]
    #[case("volume", 1.0, None)]
    #[case("volume", -1.0, None)]
    #[case("surplus", 1.0, Some(0.5))]
    #[case("surplus", -1.0, Some(0.5))]
    #[case("priceImprovement", 1.0, Some(0.5))]
    #[case("priceImprovement", -1.0, Some(0.5))]
    fn test_fee_factor_limits(
        #[case] policy: &str,
        #[case] factor: f64,
        #[case] max_factor: Option<f64>,
    ) {
        let policy = if let Some(max_factor) = max_factor {
            format!("{policy}:{factor}:{max_factor}")
        } else {
            format!("{policy}:{factor}")
        };
        assert_eq!(
            FeePolicyKind::from_str(&policy).err().unwrap(),
            format!("Factor must be in the range [0, 1), got {factor}")
        )
    }
}
