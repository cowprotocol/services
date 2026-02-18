//! Contains command line arguments and related helpers that are shared between
//! the binaries.

use {
    crate::{gas_price_estimation::GasEstimatorType, tenderly_api},
    alloy::primitives::Address,
    anyhow::{Context, Result, ensure},
    observe::TracingConfig,
    std::{
        collections::HashSet,
        fmt::{self, Display, Formatter},
        num::NonZeroU32,
        str::FromStr,
        time::Duration,
    },
    url::Url,
};

#[macro_export]
macro_rules! logging_args_with_default_filter {
    ($struct_name:ident ,$default_filter:literal) => {
        #[derive(clap::Parser)]
        pub struct $struct_name {
            #[clap(long, env, default_value = $default_filter)]
            pub log_filter: String,

            #[clap(long, env)]
            pub log_stderr_threshold: Option<tracing::Level>,

            #[clap(long, env, default_value = "false")]
            pub use_json_logs: bool,
        }

        impl ::std::fmt::Display for $struct_name {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                let Self {
                    log_filter,
                    log_stderr_threshold,
                    use_json_logs,
                } = self;

                writeln!(f, "log_filter: {}", log_filter)?;
                writeln!(f, "log_stderr_threshold: {:?}", log_stderr_threshold)?;
                writeln!(f, "use_json_logs: {}", use_json_logs)?;
                Ok(())
            }
        }
    };
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ExternalSolver {
    pub name: String,
    pub url: Url,
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
    "info,autopilot=debug,driver=debug,observe=info,orderbook=debug,solver=debug,shared=debug,\
     cow_amm=debug"
);

#[derive(Debug, clap::Parser)]
pub struct TracingArguments {
    #[clap(long, env)]
    pub tracing_collector_endpoint: Option<String>,
    #[clap(long, env, default_value_t = tracing::Level::INFO)]
    pub tracing_level: tracing::Level,
    #[clap(long, env, value_parser = humantime::parse_duration, default_value = "10s")]
    pub tracing_exporter_timeout: Duration,
}

pub fn tracing_config(args: &TracingArguments, service_name: String) -> Option<TracingConfig> {
    let Some(endpoint) = &args.tracing_collector_endpoint else {
        return None;
    };

    Some(TracingConfig::new(
        endpoint.clone(),
        service_name,
        args.tracing_exporter_timeout,
        args.tracing_level,
    ))
}

// Matches SQLx default connection pool size.
// SAFETY: 10 > 0
pub const DB_MAX_CONNECTIONS_DEFAULT: NonZeroU32 = NonZeroU32::new(10).unwrap();

#[derive(Debug, Clone, clap::Parser)]
pub struct DatabasePoolConfig {
    /// Maximum number of connections in the database connection pool.
    #[clap(long, env, default_value_t = DB_MAX_CONNECTIONS_DEFAULT)]
    pub db_max_connections: NonZeroU32,
}

impl Display for DatabasePoolConfig {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        writeln!(f, "db_max_connections: {}", self.db_max_connections)
    }
}

#[derive(clap::Parser)]
#[group(skip)]
pub struct Arguments {
    #[clap(flatten)]
    pub ethrpc: crate::web3::Arguments,

    #[clap(flatten)]
    pub current_block: crate::current_block::Arguments,

    #[clap(flatten)]
    pub tenderly: tenderly_api::Arguments,

    #[clap(flatten)]
    pub logging: LoggingArguments,

    #[clap(flatten)]
    pub tracing: TracingArguments,

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
        use_value_delimiter = true,
        value_parser = clap::value_parser!(GasEstimatorType)
    )]
    pub gas_estimators: Vec<GasEstimatorType>,

    /// The time between new blocks on the network.
    #[clap(long, env, value_parser = humantime::parse_duration)]
    pub network_block_interval: Option<Duration>,

    /// Override address of the settlement contract.
    #[clap(long, env)]
    pub settlement_contract_address: Option<Address>,

    /// Override address of the Balances contract.
    #[clap(long, env)]
    pub balances_contract_address: Option<Address>,

    /// Override address of the Signatures contract.
    #[clap(long, env)]
    pub signatures_contract_address: Option<Address>,

    /// Override address of the settlement contract.
    #[clap(long, env)]
    pub native_token_address: Option<Address>,

    /// Override the address of the `HooksTrampoline` contract used for
    /// trampolining custom order interactions. If not specified, the default
    /// contract deployment for the current network will be used.
    #[clap(long, env)]
    pub hooks_contract_address: Option<Address>,

    /// Override address of the balancer vault contract.
    #[clap(long, env)]
    pub balancer_v2_vault_address: Option<Address>,

    /// Custom volume fees for token buckets.
    /// Format: "factor:token1;token2;..." (e.g.,
    /// "0:0xA0b86...;0x6B175...;0xdAC17...") Orders where BOTH tokens are
    /// in the bucket will use the custom fee. Useful for
    /// stablecoin-to-stablecoin trades or specific token pairs (2-token
    /// buckets). Multiple buckets can be separated by commas.
    #[clap(long, env, value_delimiter = ',')]
    pub volume_fee_bucket_overrides: Vec<TokenBucketFeeOverride>,

    /// Enable volume fees for trades where sell token equals buy token.
    /// By default, volume fees are NOT applied to same-token trades.
    #[clap(long, env)]
    pub enable_sell_equals_buy_volume_fee: bool,
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
            "eip1271_onchain_quote_validity_second: {eip1271_onchain_quote_validity:?}"
        )?;
        writeln!(
            f,
            "presign_onchain_quote_validity_second: {presign_onchain_quote_validity:?}"
        )?;
        display_list(f, "price_estimation_drivers", price_estimation_drivers)?;
        writeln!(
            f,
            "standard_offchain_quote_validity: {standard_offchain_quote_validity:?}"
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
            network_block_interval,
            settlement_contract_address,
            balances_contract_address,
            signatures_contract_address,
            native_token_address,
            hooks_contract_address,
            balancer_v2_vault_address,
            tracing,
            volume_fee_bucket_overrides,
            enable_sell_equals_buy_volume_fee,
        } = self;

        write!(f, "{ethrpc}")?;
        write!(f, "{current_block}")?;
        write!(f, "{tenderly}")?;
        write!(f, "{logging}")?;
        writeln!(f, "node_url: {node_url}")?;
        display_option(f, "chain_id", chain_id)?;
        display_option(f, "simulation_node_url", simulation_node_url)?;
        writeln!(f, "gas_estimators: {gas_estimators:?}")?;
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
            "balances_contract_address",
            &balances_contract_address.map(|a| format!("{a:?}")),
        )?;
        display_option(
            f,
            "signatures_contract_address",
            &signatures_contract_address.map(|a| format!("{a:?}")),
        )?;
        display_option(
            f,
            "native_token_address",
            &native_token_address.map(|a| format!("{a:?}")),
        )?;
        display_option(
            f,
            "hooks_contract_address",
            &hooks_contract_address.map(|a| format!("{a:?}")),
        )?;
        display_option(
            f,
            "balancer_v2_vault_address",
            &balancer_v2_vault_address.map(|a| format!("{a:?}")),
        )?;
        write!(f, "{tracing:?}")?;
        writeln!(
            f,
            "volume_fee_bucket_overrides: {volume_fee_bucket_overrides:?}"
        )?;
        writeln!(
            f,
            "enable_sell_equals_buy_volume_fee: {enable_sell_equals_buy_volume_fee}"
        )?;
        Ok(())
    }
}

impl Display for ExternalSolver {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "{}({})", self.name, self.url)
    }
}

impl FromStr for ExternalSolver {
    type Err = anyhow::Error;

    fn from_str(solver: &str) -> Result<Self> {
        let parts: Vec<&str> = solver.split('|').collect();
        ensure!(
            parts.len() == 2,
            "wrong number of arguments for external solver"
        );
        let (name, url) = (parts[0], parts[1]);
        let url: Url = url.parse()?;

        Ok(Self {
            name: name.to_owned(),
            url,
        })
    }
}

/// Fee factor representing a percentage in range [0, 1)
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FeeFactor(f64);

impl FeeFactor {
    /// High precision scale factor (1 million) for sub-basis-point precision.
    /// Allows representing factors like 0.00003 (0.3 BPS) without rounding to
    /// 0. Also used for converting to BPS string with 2 decimal precision
    /// (1_000_000 / 100 = 10_000 BPS scale).
    pub const HIGH_PRECISION_SCALE: u64 = 1_000_000;

    pub fn new(factor: f64) -> Self {
        Self(factor)
    }

    /// Converts the fee factor to basis points (BPS).
    /// Supports fractional BPS values (e.g., 0.00003 -> "0.3")
    /// Rounds to 2 decimal places to avoid floating point representation
    /// issues.
    pub fn to_bps_str(&self) -> String {
        let bps = (self.0 * Self::HIGH_PRECISION_SCALE as f64).round() / 100.0;
        format!("{bps}")
    }

    /// Converts the fee factor to a high precision scaled integer.
    /// For example, 0.00003 -> 30 (with scale of 1_000_000)
    /// This allows sub-basis-point precision in calculations.
    pub fn to_high_precision(&self) -> u64 {
        (self.0 * Self::HIGH_PRECISION_SCALE as f64).round() as u64
    }

    /// Get the inner value
    pub fn get(&self) -> f64 {
        self.0
    }
}

impl TryFrom<f64> for FeeFactor {
    type Error = anyhow::Error;

    fn try_from(value: f64) -> Result<Self, Self::Error> {
        ensure!(
            (0.0..1.0).contains(&value),
            "Factor must be in the range [0, 1)"
        );
        Ok(FeeFactor(value))
    }
}

impl FromStr for FeeFactor {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let value: f64 = s.parse().context("failed to parse fee factor as f64")?;
        value.try_into()
    }
}

/// Helper type for parsing token bucket fee overrides from strings
#[derive(Debug, Clone)]
pub struct TokenBucketFeeOverride {
    pub tokens: HashSet<Address>,
    pub factor: FeeFactor,
}

impl FromStr for TokenBucketFeeOverride {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let (factor_str, tokens_str) = s.split_once(':').with_context(|| {
            format!(
                "invalid bucket override format: expected 'factor:token1;token2;...', got '{}'",
                s
            )
        })?;
        let factor = factor_str
            .parse::<f64>()
            .context("failed to parse fee factor")?
            .try_into()
            .context("fee factor out of range")?;
        let tokens: HashSet<Address> = tokens_str
            .split(';')
            .map(|token| {
                token
                    .parse::<Address>()
                    .with_context(|| format!("failed to parse token address '{}'", token))
            })
            .collect::<Result<HashSet<Address>>>()?;

        ensure!(
            tokens.len() >= 2,
            "bucket override must contain at least 2 tokens, got {}",
            tokens.len()
        );

        Ok(TokenBucketFeeOverride { tokens, factor })
    }
}

#[cfg(test)]
mod test {
    use {super::*, alloy::primitives::address};

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
    fn parse_token_bucket_fee_override() {
        // Valid inputs with 2 tokens (minimum required)
        let valid_two_tokens = "0.5:0x0000000000000000000000000000000000000001;\
                                0x0000000000000000000000000000000000000002";
        let result = TokenBucketFeeOverride::from_str(valid_two_tokens).unwrap();
        assert_eq!(result.factor.get(), 0.5);
        assert_eq!(result.tokens.len(), 2);
        assert!(
            result
                .tokens
                .contains(&address!("0000000000000000000000000000000000000001"))
        );
        assert!(
            result
                .tokens
                .contains(&address!("0000000000000000000000000000000000000002"))
        );

        // Valid inputs with 3 tokens
        let valid_three_tokens = "0.123:0x0000000000000000000000000000000000000001;\
                                  0x0000000000000000000000000000000000000002;\
                                  0x0000000000000000000000000000000000000003";
        let result = TokenBucketFeeOverride::from_str(valid_three_tokens).unwrap();
        assert_eq!(result.factor.get(), 0.123);
        assert_eq!(result.tokens.len(), 3);
        // Invalid: only 1 token (need at least 2)
        assert!(
            TokenBucketFeeOverride::from_str("0.5:0x0000000000000000000000000000000000000001")
                .is_err()
        );
        // Invalid: wrong format (no colon)
        assert!(
            TokenBucketFeeOverride::from_str("0.5,0x0000000000000000000000000000000000000001")
                .is_err()
        );
        // Invalid: too many parts
        assert!(
            TokenBucketFeeOverride::from_str(
                "0.5:0x0000000000000000000000000000000000000001:extra"
            )
            .is_err()
        );
        // Invalid: fee factor out of range
        assert!(
            TokenBucketFeeOverride::from_str("1.5:0x0000000000000000000000000000000000000001")
                .is_err()
        );
        assert!(
            TokenBucketFeeOverride::from_str("-0.1:0x0000000000000000000000000000000000000001")
                .is_err()
        );
        // Invalid: not a number for fee factor
        assert!(
            TokenBucketFeeOverride::from_str("abc:0x0000000000000000000000000000000000000001")
                .is_err()
        );
        // Invalid: bad token address
        assert!(
            TokenBucketFeeOverride::from_str(
                "0.5:notanaddress,0x0000000000000000000000000000000000000002"
            )
            .is_err()
        );
    }

    #[test]
    fn fee_factor_to_bps() {
        assert_eq!(FeeFactor::new(0.0001).to_bps_str(), "1");
        assert_eq!(FeeFactor::new(0.001).to_bps_str(), "10");

        // Fractional BPS values (sub-basis-point precision)
        assert_eq!(FeeFactor::new(0.00003).to_bps_str(), "0.3");
        assert_eq!(FeeFactor::new(0.00005).to_bps_str(), "0.5");
        assert_eq!(FeeFactor::new(0.000025).to_bps_str(), "0.25");
        assert_eq!(FeeFactor::new(0.000075).to_bps_str(), "0.75");
        assert_eq!(FeeFactor::new(0.00015).to_bps_str(), "1.5");

        assert_eq!(FeeFactor::new(0.0).to_bps_str(), "0");
    }

    #[test]
    fn fee_factor_to_high_precision() {
        // Verify high precision scaling
        assert_eq!(FeeFactor::new(0.00003).to_high_precision(), 30);
        assert_eq!(FeeFactor::new(0.0001).to_high_precision(), 100);
        assert_eq!(FeeFactor::new(0.001).to_high_precision(), 1000);
        assert_eq!(FeeFactor::new(0.01).to_high_precision(), 10_000);
        assert_eq!(FeeFactor::new(0.1).to_high_precision(), 100_000);
    }
}
