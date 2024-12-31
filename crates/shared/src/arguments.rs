//! Contains command line arguments and related helpers that are shared between
//! the binaries.

use {
    crate::{
        gas_price_estimation::GasEstimatorType,
        sources::{
            balancer_v2::BalancerFactoryKind,
            uniswap_v2::UniV2BaselineSourceParameters,
            BaselineSource,
        },
        tenderly_api,
    },
    anyhow::{ensure, Context, Result},
    bigdecimal::BigDecimal,
    ethcontract::{H160, U256},
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

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ExternalSolver {
    pub name: String,
    pub url: Url,
    pub fairness_threshold: Option<U256>,
}

// The following arguments are used to configure the order creation process
// The arguments are shared between the orderbook crate and the autopilot crate,
// as both crates can create orders
#[derive(clap::Parser)]
pub struct OrderQuotingArguments {
    /// A list of external drivers used for price estimation in the following
    /// format: `<NAME>|<URL>,<NAME>|<URL>`
    #[clap(long, env, use_value_delimiter = true)]
    pub price_estimation_drivers: Vec<ExternalSolver>,

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
    "warn,autopilot=debug,driver=debug,orderbook=debug,solver=debug,shared=debug,cow_amm=debug"
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

    /// If solvers should use internal buffers to improve solution quality.
    #[clap(long, env, action = clap::ArgAction::Set, default_value = "false")]
    pub use_internal_buffers: bool,

    /// The Balancer V2 factories to consider for indexing liquidity. Allows
    /// specific pool kinds to be disabled via configuration. Will use all
    /// supported Balancer V2 factory kinds if not specified.
    #[clap(long, env, value_enum, ignore_case = true, use_value_delimiter = true)]
    pub balancer_factories: Option<Vec<BalancerFactoryKind>>,

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

    /// The amount of time a classification of a token into good or
    /// bad is valid for.
    #[clap(
        long,
        env,
        default_value = "10m",
        value_parser = humantime::parse_duration,
    )]
    pub token_quality_cache_expiry: Duration,

    /// How long before expiry the token quality cache should try to update the
    /// token quality in the background. This is useful to make sure that token
    /// quality for every cached token is usable at all times. This value
    /// has to be smaller than `token_quality_cache_expiry`
    /// This configuration also affects the period of the token quality
    /// maintenance job. Maintenance period =
    /// `token_quality_cache_prefetch_time` / 2
    #[clap(
        long,
        env,
        default_value = "2m",
        value_parser = humantime::parse_duration,
    )]
    pub token_quality_cache_prefetch_time: Duration,
}

#[derive(Clone, clap::Parser)]
pub struct Db {
    /// Base Url of the Postgres database
    pub db_base_url: Option<Url>,
    /// Database Username
    #[clap(long, env)]
    pub db_user: Option<String>,
    /// Database password for the given username
    #[clap(long, env)]
    pub db_password: Option<String>,
}

impl Db {
    /// Returns the DB URL with credentials
    /// Returns `None` if the URL is not configured
    pub fn to_url(&self) -> Option<Url> {
        let mut url = self.db_base_url.clone()?;

        if let Some(user) = &self.db_user {
            url.query_pairs_mut()
                .append_pair("user", user)
                .extend_pairs(self.db_password.as_ref().map(|pass| ("password", pass)));
        }

        Some(url)
    }
}

pub fn display_secret_option<T>(
    f: &mut Formatter<'_>,
    name: &str,
    option: Option<&T>,
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
            price_estimation_drivers,
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
        display_list(f, "price_estimation_drivers", price_estimation_drivers)?;
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
            use_internal_buffers,
            balancer_factories,
            solver_competition_auth,
            network_block_interval,
            settlement_contract_address,
            native_token_address,
            balancer_v2_vault_address,
            custom_univ2_baseline_sources,
            liquidity_fetcher_max_age_update,
            max_pools_to_initialize_cache,
            token_quality_cache_expiry,
            token_quality_cache_prefetch_time,
        } = self;

        write!(f, "{}", ethrpc)?;
        write!(f, "{}", current_block)?;
        write!(f, "{}", tenderly)?;
        write!(f, "{}", logging)?;
        writeln!(f, "node_url: {}", node_url)?;
        display_option(f, "chain_id", chain_id)?;
        display_option(f, "simulation_node_url", simulation_node_url)?;
        writeln!(f, "gas_estimators: {:?}", gas_estimators)?;
        display_secret_option(f, "blocknative_api_key", blocknative_api_key.as_ref())?;
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
        writeln!(f, "use_internal_buffers: {}", use_internal_buffers)?;
        writeln!(f, "balancer_factories: {:?}", balancer_factories)?;
        display_secret_option(
            f,
            "solver_competition_auth",
            solver_competition_auth.as_ref(),
        )?;
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
        writeln!(
            f,
            "token_quality_cache_expiry: {:?}",
            token_quality_cache_expiry
        )?;
        writeln!(
            f,
            "token_quality_cache_prefetch_time: {:?}",
            token_quality_cache_prefetch_time
        )?;

        Ok(())
    }
}

impl Display for ExternalSolver {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "{}({})", self.name, self.url)
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
        let parts: Vec<&str> = solver.split('|').collect();
        ensure!(parts.len() >= 2, "not enough arguments for external solver");
        let (name, url) = (parts[0], parts[1]);
        let url: Url = url.parse()?;
        let fairness_threshold = match parts.get(2) {
            Some(value) => {
                Some(U256::from_dec_str(value).context("failed to parse fairness threshold")?)
            }
            None => None,
        };
        Ok(Self {
            name: name.to_owned(),
            url,
            fairness_threshold,
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
            fairness_threshold: None,
        };
        assert_eq!(driver, expected);
    }

    #[test]
    fn db_url_just_base_url() {
        let db = Db {
            db_base_url: Some("postgresql://mydatabase:1234".try_into().unwrap()),
            db_user: None,
            db_password: None,
        };

        assert_eq!(
            db.to_url(),
            Url::try_from("postgresql://mydatabase:1234").ok()
        );
    }

    #[test]
    fn db_url_base_url_with_user() {
        let db = Db {
            db_base_url: Some("postgresql://mydatabase:1234".try_into().unwrap()),
            db_user: Some("myuser".to_string()),
            db_password: None,
        };

        assert_eq!(
            db.to_url(),
            Url::try_from("postgresql://mydatabase:1234?user=myuser").ok()
        );
    }

    #[test]
    fn db_url_base_url_with_user_and_password() {
        let db = Db {
            db_base_url: Some("postgresql://mydatabase:1234".try_into().unwrap()),
            db_user: Some("myuser".to_string()),
            db_password: Some("mypassword".to_string()),
        };

        assert_eq!(
            db.to_url(),
            Url::try_from("postgresql://mydatabase:1234?user=myuser&password=mypassword").ok()
        );
    }

    #[test]
    fn db_url_empty() {
        let db = Db {
            db_base_url: None,
            db_user: None,
            db_password: None,
        };

        assert_eq!(db.to_url(), None);
    }

    #[test]
    fn parse_driver_with_threshold() {
        let argument = "name1|http://localhost:8080|1000000000000000000";
        let driver = ExternalSolver::from_str(argument).unwrap();
        let expected = ExternalSolver {
            name: "name1".into(),
            url: Url::parse("http://localhost:8080").unwrap(),
            fairness_threshold: Some(U256::exp10(18)),
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
}
