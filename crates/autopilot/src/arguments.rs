use {
    crate::{domain::fee::FeeFactor, infra},
    anyhow::Context,
    clap::ValueEnum,
    primitive_types::H160,
    shared::{
        arguments::{display_list, display_option, ExternalSolver},
        bad_token::token_owner_finder,
        http_client,
        price_estimation::{self, NativePriceEstimator, NativePriceEstimators},
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

    /// Which estimators to use as a fallback to estimate token prices in terms
    /// of the chain's native token. Estimators with the same name need to
    /// also be specified as built-in, legacy or external price estimators
    /// (lookup happens in this order in case of name collisions)
    #[clap(long, env)]
    pub native_price_estimators: NativePriceEstimators,

    /// Which estimator to primary use to estimate token prices in terms of the
    /// chain's native token.
    #[clap(long, env)]
    pub primary_native_price_estimator: Option<Vec<NativePriceEstimator>>,

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

    /// Enables multiple fees
    #[clap(long, env, action = clap::ArgAction::Set, default_value = "false")]
    pub enable_multiple_fees: bool,

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
    ///   before
    /// the deployment of the factory)
    #[clap(long, env, use_value_delimiter = true)]
    pub cow_amm_configs: Vec<CowAmmConfig>,
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
            primary_native_price_estimator,
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
            enable_multiple_fees,
            fee_policy_max_partner_fee,
            order_events_cleanup_interval,
            order_events_cleanup_threshold,
            db_url,
            insert_batch_size,
            native_price_estimation_results_required,
            auction_update_interval,
            max_settlement_transaction_wait,
            s3,
            cow_amm_configs,
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
            "primary_native_price_estimator: {:?}",
            primary_native_price_estimator
        )?;
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
        writeln!(f, "enable_multiple_fees: {:?}", enable_multiple_fees)?;
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
        writeln!(f, "auction_update_interval: {:?}", auction_update_interval)?;
        writeln!(
            f,
            "max_settlement_transaction_wait: {:?}",
            max_settlement_transaction_wait
        )?;
        writeln!(f, "s3: {:?}", s3)?;
        writeln!(f, "cow_amm_configs: {:?}", cow_amm_configs)?;
        Ok(())
    }
}

/// A fee policy to be used for orders base on it's class.
/// Examples:
/// - Surplus with a high enough cap for limit orders
/// surplus:0.5:0.9:limit
///
/// - Surplus with cap for market orders:
/// surplus:0.5:0.06:market
///
/// - Price improvement with a high enough cap for any order class
/// price_improvement:0.5:0.9:any
///
/// - Price improvement with cap for limit orders:
/// price_improvement:0.5:0.06:limit
///
/// - Volume based fee for any order class:
/// volume:0.1:any
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
    use super::*;

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
            assert!(FeePolicy::from_str(policy)
                .err()
                .unwrap()
                .to_string()
                .contains("Factor must be in the range [0, 1)"),)
        }
    }
}
