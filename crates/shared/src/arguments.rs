//! Contains command line arguments and related helpers that are shared between
//! the binaries.

use {
    crate::{
        gas_price_estimation::GasEstimatorType,
        price_estimation::PriceEstimators,
        sources::{
            balancer_v2::BalancerFactoryKind,
            uniswap_v2::UniV2BaselineSourceParameters,
            BaselineSource,
        },
        tenderly_api,
    },
    anyhow::{ensure, Context, Result},
    bigdecimal::BigDecimal,
    ethcontract::{H160, H256, U256},
    std::{
        fmt::{self, Display, Formatter},
        num::NonZeroU64,
        str::FromStr,
        time::Duration,
    },
    tracing::level_filters::LevelFilter,
    url::Url,
};

#[macro_export]
macro_rules! logging_args_with_default_filter {
    ($struct_name:ident ,$default_filter:literal) => {
        #[derive(clap::Parser)]
        pub struct $struct_name {
            #[clap(long, env, default_value = $default_filter)]
            pub log_filter: String,

            #[clap(long, env, default_value = "error")]
            pub log_stderr_threshold: LevelFilter,
        }

        impl ::std::fmt::Display for $struct_name {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                let Self {
                    log_filter,
                    log_stderr_threshold,
                } = self;

                writeln!(f, "log_filter: {}", log_filter)?;
                writeln!(f, "log_stderr_threshold: {}", log_stderr_threshold)?;
                Ok(())
            }
        }
    };
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExternalSolver {
    pub name: String,
    pub url: Url,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LegacySolver {
    pub name: String,
    pub url: Url,
    pub address: H160,
    pub use_liquidity: bool,
}

// The following arguments are used to configure the order creation process
// The arguments are shared between the orderbook crate and the autopilot crate,
// as both crates can create orders
#[derive(clap::Parser)]
pub struct OrderQuotingArguments {
    #[clap(long, env, default_value_t)]
    pub price_estimators: PriceEstimators,

    /// A list of external drivers used for price estimation in the following
    /// format: `<NAME>|<URL>,<NAME>|<URL>`
    #[clap(long, env, use_value_delimiter = true)]
    pub price_estimation_drivers: Vec<ExternalSolver>,

    /// A list of legacy solvers to be used for price estimation in the
    /// following format: `<NAME>|<URL>[|<ADDRESS>[|<USE_LIQUIITY>]]`.
    ///
    /// These solvers are used as an intermediary "transition-period" for
    /// CIP-27 for solvers that don't provide calldata and while not all
    /// quotes are verified.
    #[clap(long, env, use_value_delimiter = true)]
    pub price_estimation_legacy_solvers: Vec<LegacySolver>,

    /// The time period an EIP1271-quote request is valid.
    #[clap(
        long,
        env,
        default_value = "10m",
        value_parser = humantime::parse_duration,
    )]
    pub eip1271_onchain_quote_validity: Duration,

    /// The time period an PRESIGN-quote request is valid.
    #[clap(
        long,
        env,
        default_value = "10m",
        value_parser = humantime::parse_duration,
    )]
    pub presign_onchain_quote_validity: Duration,

    /// The time period a regular offchain-quote request (ethsign/eip712) is
    /// valid.
    #[clap(
        long,
        env,
        default_value = "1m",
        value_parser = humantime::parse_duration,
    )]
    pub standard_offchain_quote_validity: Duration,
}

logging_args_with_default_filter!(
    LoggingArguments,
    "warn,autopilot=debug,driver=debug,orderbook=debug,solver=debug,shared=debug"
);

#[derive(clap::Parser)]
#[group(skip)]
pub struct Arguments {
    #[clap(flatten)]
    pub ethrpc: crate::ethrpc::Arguments,

    #[clap(flatten)]
    pub current_block: crate::current_block::Arguments,

    #[clap(flatten)]
    pub tenderly: tenderly_api::Arguments,

    #[clap(flatten)]
    pub logging: LoggingArguments,

    /// The Ethereum node URL to connect to.
    #[clap(long, env, default_value = "http://localhost:8545")]
    pub node_url: Url,

    /// The base URL used to connect to subgraph clients.
    #[clap(long, env, default_value = "https://api.thegraph.com/subgraphs/name/")]
    pub graph_api_base_url: Url,

    /// An Ethereum node URL that supports `eth_call`s with state overrides to
    /// be used for simulations.
    #[clap(long, env)]
    pub simulation_node_url: Option<Url>,

    /// The expected chain ID that the services are expected to run against.
    /// This can be optionally specified in order to check at startup whether
    /// the connected nodes match to detect misconfigurations.
    #[clap(long, env)]
    pub chain_id: Option<u64>,

    /// Which gas estimators to use. Multiple estimators are used in sequence if
    /// a previous one fails. Individual estimators support different
    /// networks. `EthGasStation`: supports mainnet.
    /// `GasNow`: supports mainnet.
    /// `Web3`: supports every network.
    /// `Native`: supports every network.
    #[clap(
        long,
        env,
        default_value = "Web3",
        value_enum,
        ignore_case = true,
        use_value_delimiter = true
    )]
    pub gas_estimators: Vec<GasEstimatorType>,

    /// BlockNative requires api key to work. Optional since BlockNative could
    /// be skipped in gas estimators.
    #[clap(long, env)]
    pub blocknative_api_key: Option<String>,

    /// Base tokens used for finding multi-hop paths between multiple AMMs
    /// Should be the most liquid tokens of the given network.
    #[clap(long, env, use_value_delimiter = true)]
    pub base_tokens: Vec<H160>,

    /// Which Liquidity sources to be used by Price Estimator.
    #[clap(long, env, value_enum, ignore_case = true, use_value_delimiter = true)]
    pub baseline_sources: Option<Vec<BaselineSource>>,

    /// List of non hardcoded univ2-like contracts.
    ///
    /// For example to add a univ2-like liquidity source the argument could be
    /// set to
    ///
    /// 0x0000000000000000000000000000000000000001|0x0000000000000000000000000000000000000000000000000000000000000002
    ///
    /// which sets the router address to 0x01 and the init code digest to 0x02.
    #[clap(long, env, value_enum, ignore_case = true, use_value_delimiter = true)]
    pub custom_univ2_baseline_sources: Vec<UniV2BaselineSourceParameters>,

    /// The number of blocks kept in the pool cache.
    #[clap(long, env, default_value = "10")]
    pub pool_cache_blocks: NonZeroU64,

    /// The number of pairs that are automatically updated in the pool cache.
    #[clap(long, env, default_value = "4")]
    pub pool_cache_maximum_recent_block_age: u64,

    /// How often to retry requests in the pool cache.
    #[clap(long, env, default_value = "5")]
    pub pool_cache_maximum_retries: u32,

    /// How long to sleep in seconds between retries in the pool cache.
    #[clap(long, env, default_value = "1s", value_parser = humantime::parse_duration)]
    pub pool_cache_delay_between_retries: Duration,

    /// The ParaSwap API base url to use.
    #[clap(long, env, default_value = super::paraswap_api::DEFAULT_URL)]
    pub paraswap_api_url: String,

    /// Special partner authentication for Paraswap API (allowing higher rater
    /// limits)
    #[clap(long, env)]
    pub paraswap_partner: Option<String>,

    /// The list of disabled ParaSwap DEXs. By default, the `ParaSwapPool4`
    /// DEX (representing a private market maker) is disabled as it increases
    /// price by 1% if built transactions don't actually get executed.
    #[clap(long, env, default_value = "ParaSwapPool4", use_value_delimiter = true)]
    pub disabled_paraswap_dexs: Vec<String>,

    #[clap(long, env)]
    pub zeroex_url: Option<String>,

    #[clap(long, env)]
    pub zeroex_api_key: Option<String>,

    /// If solvers should use internal buffers to improve solution quality.
    #[clap(long, env, action = clap::ArgAction::Set, default_value = "false")]
    pub use_internal_buffers: bool,

    /// The Balancer V2 factories to consider for indexing liquidity. Allows
    /// specific pool kinds to be disabled via configuration. Will use all
    /// supported Balancer V2 factory kinds if not specified.
    #[clap(long, env, value_enum, ignore_case = true, use_value_delimiter = true)]
    pub balancer_factories: Option<Vec<BalancerFactoryKind>>,

    /// The list of disabled 1Inch protocols. By default, the `PMM1` protocol
    /// (representing a private market maker) is disabled as it seems to
    /// produce invalid swaps.
    #[clap(long, env, default_value = "PMM1", use_value_delimiter = true)]
    pub disabled_one_inch_protocols: Vec<String>,

    /// Which address should receive the rewards for referring trades to 1Inch.
    #[structopt(long, env)]
    pub one_inch_referrer_address: Option<H160>,

    /// The list of disabled 0x sources.
    #[clap(long, env, use_value_delimiter = true)]
    pub disabled_zeroex_sources: Vec<String>,

    /// Deny list of balancer pool ids.
    #[clap(long, env, use_value_delimiter = true)]
    pub balancer_pool_deny_list: Vec<H256>,

    /// Value of the authorization header for the solver competition post api.
    #[clap(long, env)]
    pub solver_competition_auth: Option<String>,

    /// If liquidity pool fetcher has caching mechanism, this argument defines
    /// how old pool data is allowed to be before updating
    #[clap(
        long,
        env,
        default_value = "30s",
        value_parser = humantime::parse_duration,
    )]
    pub liquidity_fetcher_max_age_update: Duration,

    /// The number of pools to initially populate the UniswapV3 cache
    #[clap(long, env, default_value = "100")]
    pub max_pools_to_initialize_cache: usize,

    /// The time between new blocks on the network.
    #[clap(long, env, value_parser = humantime::parse_duration)]
    pub network_block_interval: Option<Duration>,

    /// Override address of the settlement contract.
    #[clap(long, env)]
    pub settlement_contract_address: Option<H160>,

    /// Override address of the settlement contract.
    #[clap(long, env)]
    pub native_token_address: Option<H160>,

    /// Override address of the balancer vault contract.
    #[clap(long, env)]
    pub balancer_v2_vault_address: Option<H160>,

    /// Deprecate market orders (orders with positive signed fee) starting from
    /// date
    #[clap(long, env)]
    pub market_orders_deprecation_date: Option<chrono::DateTime<chrono::Utc>>,
}

/// The kind of EVM code simulator to use.
#[derive(Copy, Clone, Debug, Hash, Eq, PartialEq, clap::ValueEnum)]
#[clap(rename_all = "verbatim")]
pub enum CodeSimulatorKind {
    Web3,
    Tenderly,
    Web3ThenTenderly,
}

pub fn display_secret_option<T>(
    f: &mut Formatter<'_>,
    name: &str,
    option: &Option<T>,
) -> std::fmt::Result {
    display_option(f, name, &option.as_ref().map(|_| "SECRET"))
}

pub fn display_option(
    f: &mut Formatter<'_>,
    name: &str,
    option: &Option<impl Display>,
) -> std::fmt::Result {
    write!(f, "{name}: ")?;
    match option {
        Some(display) => writeln!(f, "{display}"),
        None => writeln!(f, "None"),
    }
}

pub fn display_list<T>(
    f: &mut Formatter<'_>,
    name: &str,
    iter: impl IntoIterator<Item = T>,
) -> std::fmt::Result
where
    T: Display,
{
    write!(f, "{name}: [")?;
    for (i, t) in iter.into_iter().enumerate() {
        if i != 0 {
            f.write_str(", ")?;
        }
        write!(f, "{t}")?;
    }
    writeln!(f, "]")?;
    Ok(())
}

impl Display for OrderQuotingArguments {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let Self {
            eip1271_onchain_quote_validity,
            presign_onchain_quote_validity,
            price_estimators,
            price_estimation_drivers,
            price_estimation_legacy_solvers,
            standard_offchain_quote_validity,
        } = self;

        writeln!(
            f,
            "eip1271_onchain_quote_validity_second: {:?}",
            eip1271_onchain_quote_validity
        )?;
        writeln!(
            f,
            "presign_onchain_quote_validity_second: {:?}",
            presign_onchain_quote_validity
        )?;
        writeln!(f, "price_estimators: {}", price_estimators)?;
        display_list(f, "price_estimation_drivers", price_estimation_drivers)?;
        display_list(
            f,
            "price_estimation_legacy_solvers",
            price_estimation_legacy_solvers,
        )?;
        writeln!(
            f,
            "standard_offchain_quote_validity: {:?}",
            standard_offchain_quote_validity
        )?;
        Ok(())
    }
}
// We have a custom Display implementation so that we can log the arguments on
// start up without leaking any potentially secret values.
impl Display for Arguments {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        let Self {
            ethrpc,
            current_block,
            tenderly,
            logging,
            node_url,
            graph_api_base_url,
            chain_id,
            simulation_node_url,
            gas_estimators,
            blocknative_api_key,
            base_tokens,
            baseline_sources,
            pool_cache_blocks,
            pool_cache_maximum_recent_block_age,
            pool_cache_maximum_retries,
            pool_cache_delay_between_retries,
            paraswap_partner,
            disabled_paraswap_dexs,
            zeroex_url,
            zeroex_api_key,
            use_internal_buffers,
            balancer_factories,
            disabled_one_inch_protocols,
            one_inch_referrer_address,
            disabled_zeroex_sources,
            balancer_pool_deny_list,
            solver_competition_auth,
            network_block_interval,
            settlement_contract_address,
            native_token_address,
            balancer_v2_vault_address,
            custom_univ2_baseline_sources,
            paraswap_api_url,
            liquidity_fetcher_max_age_update,
            max_pools_to_initialize_cache,
            market_orders_deprecation_date,
        } = self;

        write!(f, "{}", ethrpc)?;
        write!(f, "{}", current_block)?;
        write!(f, "{}", tenderly)?;
        write!(f, "{}", logging)?;
        writeln!(f, "node_url: {}", node_url)?;
        writeln!(f, "graph_api_base_url: {}", graph_api_base_url)?;
        display_option(f, "chain_id", chain_id)?;
        display_option(f, "simulation_node_url", simulation_node_url)?;
        writeln!(f, "gas_estimators: {:?}", gas_estimators)?;
        display_secret_option(f, "blocknative_api_key", blocknative_api_key)?;
        writeln!(f, "base_tokens: {:?}", base_tokens)?;
        writeln!(f, "baseline_sources: {:?}", baseline_sources)?;
        writeln!(f, "pool_cache_blocks: {}", pool_cache_blocks)?;
        writeln!(
            f,
            "pool_cache_maximum_recent_block_age: {}",
            pool_cache_maximum_recent_block_age
        )?;
        writeln!(
            f,
            "pool_cache_maximum_retries: {}",
            pool_cache_maximum_retries
        )?;
        writeln!(
            f,
            "pool_cache_delay_between_retries: {:?}",
            pool_cache_delay_between_retries
        )?;
        display_secret_option(f, "paraswap_partner", paraswap_partner)?;
        display_list(f, "disabled_paraswap_dexs", disabled_paraswap_dexs)?;
        display_option(f, "zeroex_url", zeroex_url)?;
        display_secret_option(f, "zeroex_api_key", zeroex_api_key)?;
        writeln!(f, "use_internal_buffers: {}", use_internal_buffers)?;
        writeln!(f, "balancer_factories: {:?}", balancer_factories)?;
        display_list(
            f,
            "disabled_one_inch_protocols",
            disabled_one_inch_protocols,
        )?;
        display_option(
            f,
            "one_inch_referrer_address",
            &one_inch_referrer_address.map(|a| format!("{a:?}")),
        )?;
        display_list(f, "disabled_zeroex_sources", disabled_zeroex_sources)?;
        writeln!(f, "balancer_pool_deny_list: {:?}", balancer_pool_deny_list)?;
        display_secret_option(f, "solver_competition_auth", solver_competition_auth)?;
        display_option(
            f,
            "network_block_interval",
            &network_block_interval.map(|duration| duration.as_secs_f32()),
        )?;
        display_option(
            f,
            "settlement_contract_address",
            &settlement_contract_address.map(|a| format!("{a:?}")),
        )?;
        display_option(
            f,
            "native_token_address",
            &native_token_address.map(|a| format!("{a:?}")),
        )?;
        display_option(
            f,
            "balancer_v2_vault_address",
            &balancer_v2_vault_address.map(|a| format!("{a:?}")),
        )?;
        display_list(
            f,
            "custom_univ2_baseline_sources",
            custom_univ2_baseline_sources,
        )?;
        writeln!(f, "paraswap_api_url: {}", paraswap_api_url)?;
        writeln!(
            f,
            "liquidity_fetcher_max_age_update: {:?}",
            liquidity_fetcher_max_age_update
        )?;
        writeln!(
            f,
            "max_pools_to_initialize_cache: {}",
            max_pools_to_initialize_cache
        )?;
        display_option(
            f,
            "market_orders_deprecation_date",
            market_orders_deprecation_date,
        )?;

        Ok(())
    }
}

impl Display for ExternalSolver {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "{}({})", self.name, self.url)
    }
}

impl Display for LegacySolver {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "{}({}, {:?})", self.name, self.url, self.address)
    }
}

pub fn parse_percentage_factor(s: &str) -> Result<f64> {
    let percentage_factor = f64::from_str(s)?;
    ensure!(percentage_factor.is_finite() && (0. ..=1.0).contains(&percentage_factor));
    Ok(percentage_factor)
}

pub fn wei_from_ether(s: &str) -> anyhow::Result<U256> {
    let in_ether = s.parse::<BigDecimal>()?;
    let base = BigDecimal::new(1.into(), -18);
    number::conversions::big_decimal_to_u256(&(in_ether * base)).context("invalid Ether value")
}

pub fn wei_from_gwei(s: &str) -> anyhow::Result<f64> {
    let in_gwei: f64 = s.parse()?;
    Ok(in_gwei * 1e9)
}

impl FromStr for ExternalSolver {
    type Err = anyhow::Error;

    fn from_str(solver: &str) -> Result<Self> {
        let (name, url) = solver
            .split_once('|')
            .context("not enough arguments for external solver")?;
        let url: Url = url.parse()?;
        Ok(Self {
            name: name.to_owned(),
            url,
        })
    }
}

impl FromStr for LegacySolver {
    type Err = anyhow::Error;

    fn from_str(solver: &str) -> Result<Self> {
        let mut parts = solver.splitn(4, '|');
        let name = parts.next().context("missing name for legacy solver")?;
        let url = parts.next().context("missing url for legacy solver")?;
        let address = parts
            .next()
            .unwrap_or("0x0000000000000000000000000000000000000000");
        let use_liquidity = parts.next().unwrap_or("false");
        Ok(Self {
            name: name.to_owned(),
            url: url.parse()?,
            address: address.parse()?,
            use_liquidity: use_liquidity.parse()?,
        })
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn parse_driver() {
        let argument = "name1|http://localhost:8080";
        let driver = ExternalSolver::from_str(argument).unwrap();
        let expected = ExternalSolver {
            name: "name1".into(),
            url: Url::parse("http://localhost:8080").unwrap(),
        };
        assert_eq!(driver, expected);
    }

    #[test]
    fn parse_drivers_wrong_arguments() {
        // too few arguments
        assert!(ExternalSolver::from_str("").is_err());
        assert!(ExternalSolver::from_str("name").is_err());

        // broken URL
        assert!(ExternalSolver::from_str("name1|sdfsdfds").is_err());

        // too many arguments
        assert!(
            ExternalSolver::from_str("name1|http://localhost:8080|additional_argument").is_err()
        );
    }

    #[test]
    fn parse_legacy_solver_price_estimators() {
        // ok
        assert_eq!(
            LegacySolver::from_str("name|http://localhost:8080").unwrap(),
            LegacySolver {
                name: "name".to_string(),
                url: "http://localhost:8080".parse().unwrap(),
                address: H160::zero(),
                use_liquidity: false,
            }
        );
        assert_eq!(
            LegacySolver::from_str(
                "name|http://localhost:8080|0x0101010101010101010101010101010101010101"
            )
            .unwrap(),
            LegacySolver {
                name: "name".to_string(),
                url: "http://localhost:8080".parse().unwrap(),
                address: H160([1; 20]),
                use_liquidity: false,
            }
        );
        assert_eq!(
            LegacySolver::from_str(
                "name|http://localhost:8080|0x0101010101010101010101010101010101010101|true"
            )
            .unwrap(),
            LegacySolver {
                name: "name".to_string(),
                url: "http://localhost:8080".parse().unwrap(),
                address: H160([1; 20]),
                use_liquidity: true,
            }
        );

        // too few arguments
        assert!(LegacySolver::from_str("").is_err());
        assert!(LegacySolver::from_str("name").is_err());

        // broken URL
        assert!(LegacySolver::from_str("name1|sdfsdfds").is_err());

        // too many arguments
        assert!(LegacySolver::from_str(
            "name|http://localhost:8080|0x0101010101010101010101010101010101010101|true|1"
        )
        .is_err());
    }
}
