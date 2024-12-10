use {
    crate::domain::fee::FeeFactor,
    anyhow::Context,
    primitive_types::H160,
    reqwest::Url,
    shared::{
        arguments::{display_option, display_secret_option},
        bad_token::token_owner_finder,
        http_client,
        price_estimation::{self, NativePriceEstimators},
    },
    std::{net::SocketAddr, num::NonZeroUsize, str::FromStr, time::Duration},
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

    /// A tracing Ethereum node URL to connect to, allowing a separate node URL
    /// to be used exclusively for tracing calls.
    #[clap(long, env)]
    pub tracing_node_url: Option<Url>,

    #[clap(long, env, default_value = "0.0.0.0:8080")]
    pub bind_address: SocketAddr,

    /// Url of the Postgres database. By default connects to locally running
    /// postgres.
    #[clap(long, env, default_value = "postgresql://")]
    pub db_url: Url,

    /// The minimum amount of time in seconds an order has to be valid for.
    #[clap(
        long,
        env,
        default_value = "1m",
        value_parser = humantime::parse_duration,
    )]
    pub min_order_validity_period: Duration,

    /// The maximum amount of time in seconds an order can be valid for.
    /// Defaults to 3 hours. This restriction does not apply to liquidity
    /// owner orders or presign orders.
    #[clap(
        long,
        env,
        default_value = "3h",
        value_parser = humantime::parse_duration,
    )]
    pub max_order_validity_period: Duration,

    /// The maximum amount of time in seconds a limit order can be valid for.
    /// Defaults to 1 year.
    #[clap(
        long,
        env,
        default_value = "1y",
        value_parser = humantime::parse_duration,
    )]
    pub max_limit_order_validity_period: Duration,

    /// List of token addresses to be ignored throughout service
    #[clap(long, env, use_value_delimiter = true)]
    pub unsupported_tokens: Vec<H160>,

    /// List of account addresses to be denied from order creation
    #[clap(long, env, use_value_delimiter = true)]
    pub banned_users: Vec<H160>,

    /// Which estimators to use to estimate token prices in terms of the chain's
    /// native token.
    #[clap(long, env)]
    pub native_price_estimators: NativePriceEstimators,

    /// How many successful price estimates for each order will cause a fast
    /// or native price estimation to return its result early.
    /// The bigger the value the more the fast price estimation performs like
    /// the optimal price estimation.
    /// It's possible to pass values greater than the total number of enabled
    /// estimators but that will not have any further effect.
    #[clap(long, env, default_value = "2")]
    pub fast_price_estimation_results_required: NonZeroUsize,

    /// List of token addresses that should be allowed regardless of whether the
    /// bad token detector thinks they are bad. Base tokens are
    /// automatically allowed.
    #[clap(long, env, use_value_delimiter = true)]
    pub allowed_tokens: Vec<H160>,

    /// The number of pairs that are automatically updated in the pool cache.
    #[clap(long, env, default_value = "200")]
    pub pool_cache_lru_size: NonZeroUsize,

    /// Skip EIP-1271 order signature validation on creation.
    #[clap(long, env, action = clap::ArgAction::Set, default_value = "false")]
    pub eip1271_skip_creation_validation: bool,

    /// If solvable orders haven't been successfully updated in this many blocks
    /// attempting to get them errors and our liveness check fails.
    #[clap(long, env, default_value = "24")]
    pub solvable_orders_max_update_age_blocks: u64,

    /// Max number of limit orders per user.
    #[clap(long, env, default_value = "10")]
    pub max_limit_orders_per_user: u64,

    /// If set, the orderbook will use this IPFS gateway to fetch full app data
    /// for orders that only specify the contract app data hash.
    #[clap(long, env)]
    pub ipfs_gateway: Option<Url>,

    /// Authentication key for Pinata IPFS gateway.
    #[clap(long, env)]
    pub ipfs_pinata_auth: Option<String>,

    /// Override the address of the `HooksTrampoline` contract used for
    /// trampolining custom order interactions. If not specified, the default
    /// contract deployment for the current network will be used.
    #[clap(long, env)]
    pub hooks_contract_address: Option<H160>,

    /// Set the maximum size in bytes of order app data.
    #[clap(long, env, default_value = "8192")]
    pub app_data_size_limit: usize,

    /// The maximum gas amount a single order can use for getting settled.
    #[clap(long, env, default_value = "8000000")]
    pub max_gas_per_order: u64,

    /// Configurations for indexing CoW AMMs. Supplied in the form of:
    /// "<factory1>|<helper1>|<block1>,<factory2>|<helper2>,<block2>"
    /// - factory is contract address emmiting CoW AMM deployment events.
    /// - helper is a contract address to interface with pools deployed by the
    ///   factory
    /// - block is the block at which indexing should start (should be 1 block
    ///   before the deployment of the factory)
    #[clap(long, env, use_value_delimiter = true)]
    pub cow_amm_configs: Vec<CowAmmConfig>,

    /// Archive node URL used to index CoW AMM
    #[clap(long, env)]
    pub archive_node_url: Option<Url>,

    /// Describes how the protocol fees should be calculated.
    #[clap(long, env, use_value_delimiter = true)]
    pub fee_policies: Vec<FeePolicy>,
    
    /// Maximum partner fee allow. If the partner fee specified is greater than
    /// this maximum, the partner fee will be capped
    #[clap(long, env, default_value = "0.01")]
    pub fee_policy_max_partner_fee: FeeFactor,
}

impl std::fmt::Display for Arguments {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        let Arguments {
            shared,
            order_quoting,
            http_client,
            token_owner_finder,
            price_estimation,
            tracing_node_url,
            bind_address,
            min_order_validity_period,
            max_order_validity_period,
            max_limit_order_validity_period,
            unsupported_tokens,
            banned_users,
            allowed_tokens,
            pool_cache_lru_size,
            eip1271_skip_creation_validation,
            solvable_orders_max_update_age_blocks,
            native_price_estimators,
            fast_price_estimation_results_required,
            max_limit_orders_per_user,
            ipfs_gateway,
            ipfs_pinata_auth,
            hooks_contract_address,
            app_data_size_limit,
            db_url,
            max_gas_per_order,
            cow_amm_configs,
            archive_node_url,
            fee_policies,
            fee_policy_max_partner_fee,
        } = self;

        write!(f, "{}", shared)?;
        write!(f, "{}", order_quoting)?;
        write!(f, "{}", http_client)?;
        write!(f, "{}", token_owner_finder)?;
        write!(f, "{}", price_estimation)?;
        display_option(f, "tracing_node_url", tracing_node_url)?;
        writeln!(f, "bind_address: {}", bind_address)?;
        let _intentionally_ignored = db_url;
        writeln!(f, "db_url: SECRET")?;
        writeln!(
            f,
            "min_order_validity_period: {:?}",
            min_order_validity_period
        )?;
        writeln!(
            f,
            "max_order_validity_period: {:?}",
            max_order_validity_period
        )?;
        writeln!(
            f,
            "max_limit_order_validity_period: {:?}",
            max_limit_order_validity_period
        )?;
        writeln!(f, "unsupported_tokens: {:?}", unsupported_tokens)?;
        writeln!(f, "banned_users: {:?}", banned_users)?;
        writeln!(f, "allowed_tokens: {:?}", allowed_tokens)?;
        writeln!(f, "pool_cache_lru_size: {}", pool_cache_lru_size)?;
        writeln!(
            f,
            "eip1271_skip_creation_validation: {}",
            eip1271_skip_creation_validation
        )?;
        writeln!(
            f,
            "solvable_orders_max_update_age_blocks: {}",
            solvable_orders_max_update_age_blocks,
        )?;
        writeln!(f, "native_price_estimators: {}", native_price_estimators)?;
        writeln!(
            f,
            "fast_price_estimation_results_required: {}",
            fast_price_estimation_results_required
        )?;
        writeln!(
            f,
            "max_limit_orders_per_user: {}",
            max_limit_orders_per_user
        )?;
        writeln!(f, "ipfs_gateway: {:?}", ipfs_gateway)?;
        display_secret_option(f, "ipfs_pinata_auth", ipfs_pinata_auth.as_ref())?;
        display_option(
            f,
            "hooks_contract_address",
            &hooks_contract_address.map(|a| format!("{a:?}")),
        )?;
        writeln!(f, "app_data_size_limit: {}", app_data_size_limit)?;
        writeln!(f, "max_gas_per_order: {}", max_gas_per_order)?;

        Ok(())
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
