use {
    crate::{
        banned_users::BannedUsersConfig,
        database::DatabasePoolConfig,
        fee_factor::FeeFactor,
        http_client::HttpClient,
        order_quoting::OrderQuoting,
        orderbook::{
            ipfs::IpfsConfig,
            native_price::NativePriceConfig,
            order_validation::OrderValidationConfig,
        },
        price_estimation::PriceEstimation,
        shared::SharedConfig,
    },
    alloy::primitives::{Address, U256},
    anyhow::anyhow,
    chrono::{DateTime, Utc},
    serde::{Deserialize, Serialize},
    std::{
        net::{Ipv4Addr, SocketAddr, SocketAddrV4},
        path::Path,
    },
};

pub mod ipfs;
pub mod native_price;
pub mod order_validation;

const fn default_bind_address() -> SocketAddr {
    SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::UNSPECIFIED, 8080))
}

const fn default_app_data_size_limit() -> usize {
    8192
}

const fn default_active_order_competition_threshold() -> u32 {
    5
}

/// Volume-based protocol fee applied to orders.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct VolumeFeeConfig {
    /// Fee as a fraction of the order volume (e.g. 0.0002 = 0.02%).
    pub factor: Option<FeeFactor>,
    /// Timestamp from which this fee configuration becomes effective.
    pub effective_from_timestamp: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct OrderSimulationConfig {
    pub gas_limit: U256,

    /// Optional Tenderly configuration. When set, simulations are automatically
    /// submitted and shared on Tenderly, and the response includes a dashboard
    /// URL.
    #[serde(default)]
    pub tenderly: Option<crate::simulator::TenderlyConfig>,
}

/// Top-level orderbook service configuration.
// NOTE: cannot add deny_unknown_fields during the config migration
#[derive(Debug, Deserialize)]
#[cfg_attr(any(test, feature = "test-util"), derive(serde::Serialize))]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct Configuration {
    /// Shared settings (node URL, gas estimators, logging, etc.).
    #[serde(default)]
    pub shared: SharedConfig,

    /// Bind address for the Orderbook.
    #[serde(default = "default_bind_address")]
    pub bind_address: SocketAddr,

    /// Configuration for the order validation system.
    #[serde(default)]
    pub order_validation: OrderValidationConfig,

    /// Configuration for the banned users mechanism,
    /// such as their addresses and the size of the banned user's cache.
    #[serde(default)]
    pub banned_users: BannedUsersConfig,

    /// Configuration for the IPFS gateway, supports token authentication.
    pub ipfs: Option<IpfsConfig>,

    /// Configuration for the volume fee.
    pub volume_fee: Option<VolumeFeeConfig>,

    /// Maximum allowed size (in bytes) for order app-data.
    #[serde(default = "default_app_data_size_limit")]
    pub app_data_size_limit: usize,

    /// Missed auctions threshold after which an order is no longer "active".
    #[serde(default = "default_active_order_competition_threshold")]
    pub active_order_competition_threshold: u32,

    /// Denylist of tokens (due to lack of support, or other reason).
    #[serde(default)]
    pub unsupported_tokens: Vec<Address>,

    /// Whether to skip EIP-1271 signature validation.
    #[serde(default)]
    pub eip1271_skip_creation_validation: bool,

    /// Configuration for the native price estimation mechanism.
    pub native_price_estimation: NativePriceConfig,

    /// Database connection pool settings.
    #[serde(default)]
    pub database: DatabasePoolConfig,

    /// Configurations for the HTTP client (e.g. HTTP timeout).
    #[serde(default)]
    pub http_client: HttpClient,

    /// Configurations for the order creation process.
    pub order_quoting: OrderQuoting,

    /// Configurations for price estimation (tenderly, rate limiting, CoinGecko,
    /// 1inch, quote verification, balance overrides, etc.).
    #[serde(default)]
    pub price_estimation: PriceEstimation,

    /// Order simulation configuration. If `None`, the endpoint is disabled.
    #[serde(default)]
    pub order_simulation: Option<OrderSimulationConfig>,

    /// When enabled, solver competition endpoints return 404 until the
    /// auction's submission deadline block has been reached.
    #[serde(default)]
    pub hide_competition_before_deadline: bool,
}

impl Configuration {
    pub async fn from_path<P: AsRef<Path>>(path: P) -> anyhow::Result<Self> {
        match toml::from_str(&tokio::fs::read_to_string(&path).await?) {
            Ok(self_) => Ok(self_),
            Err(err) if std::env::var("TOML_TRACE_ERROR").is_ok_and(|v| v == "1") => Err(anyhow!(
                "failed to parse TOML config at {}: {err:#?}",
                path.as_ref().display()
            )),
            Err(_) => Err(anyhow!(
                "failed to parse TOML config at: {}. Set TOML_TRACE_ERROR=1 to print parsing \
                 error but this may leak secrets.",
                path.as_ref().display()
            )),
        }
    }
}

#[cfg(any(test, feature = "test-util"))]
pub mod test_util {
    use {
        crate::{
            orderbook::{
                Configuration,
                OrderSimulationConfig,
                default_active_order_competition_threshold,
                default_app_data_size_limit,
                default_bind_address,
                native_price::NativePriceConfig,
            },
            price_estimation::PriceEstimation,
            test_util::TestDefault,
        },
        alloy::primitives::U256,
        std::path::Path,
    };

    impl Configuration {
        pub async fn to_path<P: AsRef<Path>>(&self, path: P) -> anyhow::Result<()> {
            Ok(tokio::fs::write(path, toml::to_string_pretty(self)?).await?)
        }

        pub fn to_temp_path(&self) -> tempfile::NamedTempFile {
            use std::io::Write;
            let mut file =
                tempfile::NamedTempFile::new().expect("temp file creation should not fail");
            file.write_all(
                toml::to_string_pretty(self)
                    .expect("serialization should not fail")
                    .as_bytes(),
            )
            .expect("writing to temp file should not fail");
            file
        }

        pub fn to_cli_args(&self) -> (tempfile::NamedTempFile, String) {
            let named_temp_file = self.to_temp_path();
            let cli_arg = format!("--config={}", named_temp_file.path().display());
            (named_temp_file, cli_arg)
        }
    }

    impl TestDefault for Configuration {
        fn test_default() -> Self {
            use crate::test_util::TestDefault;

            Self {
                shared: Default::default(),
                bind_address: default_bind_address(),
                order_validation: Default::default(),
                banned_users: Default::default(),
                ipfs: Default::default(),
                volume_fee: Default::default(),
                app_data_size_limit: default_app_data_size_limit(),
                active_order_competition_threshold: default_active_order_competition_threshold(),
                unsupported_tokens: Default::default(),
                eip1271_skip_creation_validation: Default::default(),
                // NOTE: NativePriceConfig needs to be moved to the config crate and then it can
                // have the test_default trait impl
                native_price_estimation: NativePriceConfig::test_default(),
                database: TestDefault::test_default(),
                http_client: Default::default(),
                order_quoting: TestDefault::test_default(),
                price_estimation: PriceEstimation::test_default(),
                // Enable order simulation for testing
                order_simulation: Some(OrderSimulationConfig {
                    gas_limit: U256::try_from(16777215).expect("u64 can be converted to U256"),
                    tenderly: None,
                }),
                hide_competition_before_deadline: false,
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        crate::{
            native_price_estimators::{NativePriceEstimator, NativePriceEstimators},
            orderbook::order_validation::SameTokensPolicy,
            test_util::TestDefault,
        },
        std::time::Duration,
    };

    #[test]
    fn deserialize_full_configuration() {
        let toml = r#"
        app-data-size-limit = 4096
        active-order-competition-threshold = 10
        unsupported-tokens = ["0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2"]
        eip1271-skip-creation-validation = true
        hide-competition-before-deadline = true

        [banned-users]
        addresses = ["0xdead000000000000000000000000000000000000"]

        [order-validation]
        min-order-validity-period = "2m"
        max-order-validity-period = "6h"
        max-limit-order-validity-period = "30d"
        max-limit-orders-per-user = 5
        max-gas-per-order = 5000000
        same-tokens-policy = "allow-sell"

        [ipfs]
        gateway = "https://gateway.pinata.cloud/ipfs/"
        auth-token = "my-secret-key"

        [volume-fee]
        factor = 0.0002
        effective-from-timestamp = "2025-06-01T00:00:00Z"

        [native-price-estimation]
        estimators = [[{type = "CoinGecko"}]]

        [order-quoting]
        price-estimation-drivers = []

        [order-simulation]
        gas-limit = "123456789"
        "#;

        let config: Configuration = toml::from_str(toml).unwrap();

        assert_eq!(config.app_data_size_limit, 4096);
        assert_eq!(config.active_order_competition_threshold, 10);
        assert_eq!(config.unsupported_tokens.len(), 1);
        assert_eq!(config.banned_users.addresses.len(), 1);
        assert!(config.eip1271_skip_creation_validation);
        assert!(config.hide_competition_before_deadline);
        assert_eq!(
            config.order_simulation.map(|config| config.gas_limit),
            Some(U256::from(123456789u64))
        );

        assert!(matches!(
            config.order_validation.same_tokens_policy,
            SameTokensPolicy::AllowSell
        ));
        assert_eq!(config.order_validation.max_limit_orders_per_user, 5);

        assert_eq!(
            config.order_validation.min_order_validity_period,
            Duration::from_secs(120)
        );
        assert_eq!(
            config.order_validation.max_order_validity_period,
            Duration::from_secs(21600)
        );
        assert_eq!(
            config.order_validation.max_limit_order_validity_period,
            Duration::from_secs(2_592_000)
        );

        let ipfs = config.ipfs.unwrap();
        assert_eq!(ipfs.gateway.as_str(), "https://gateway.pinata.cloud/ipfs/");
        assert_eq!(ipfs.auth_token.unwrap(), "my-secret-key");

        let vol_fee = config.volume_fee.unwrap();
        assert_eq!(vol_fee.factor.unwrap().get(), 0.0002);
        assert!(vol_fee.effective_from_timestamp.is_some());
    }

    #[test]
    fn deserialize_configuration_defaults() {
        let toml = r#"
        [native-price-estimation]
        estimators = [[{type = "CoinGecko"}]]

        [order-quoting]
        price-estimation-drivers = []
        "#;
        let config: Configuration = toml::from_str(toml).unwrap();

        assert_eq!(config.app_data_size_limit, 8192);
        assert_eq!(config.active_order_competition_threshold, 5);

        assert_eq!(
            config.order_validation.min_order_validity_period,
            Duration::from_secs(60)
        );
        assert_eq!(
            config.order_validation.max_order_validity_period,
            Duration::from_secs(10800)
        );
        assert_eq!(
            config.order_validation.max_limit_order_validity_period,
            Duration::from_secs(31_536_000)
        );

        assert!(config.ipfs.is_none());
        assert!(config.volume_fee.is_none());
        assert!(config.unsupported_tokens.is_empty());
        assert!(config.banned_users.addresses.is_empty());
        assert!(!config.eip1271_skip_creation_validation);
        assert!(!config.hide_competition_before_deadline);
    }

    #[test]
    fn roundtrip_serialization() {
        let config = Configuration {
            shared: Default::default(),
            bind_address: default_bind_address(),
            order_validation: OrderValidationConfig {
                min_order_validity_period: Duration::from_secs(120),
                max_order_validity_period: Duration::from_secs(7200),
                max_limit_order_validity_period: Duration::from_secs(86400),
                max_limit_orders_per_user: 5,
                max_gas_per_order: 6_000_000,
                same_tokens_policy: SameTokensPolicy::AllowSell,
            },
            ipfs: Some(IpfsConfig {
                gateway: "https://gateway.pinata.cloud/ipfs/".parse().unwrap(),
                auth_token: Some("secret".to_string()),
            }),
            app_data_size_limit: 4096,
            active_order_competition_threshold: 10,
            volume_fee: Some(VolumeFeeConfig {
                factor: Some(0.0002.try_into().unwrap()),
                effective_from_timestamp: None,
            }),
            unsupported_tokens: vec![
                "0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2"
                    .parse()
                    .unwrap(),
            ],
            banned_users: Default::default(),
            eip1271_skip_creation_validation: true,
            hide_competition_before_deadline: true,
            native_price_estimation: NativePriceConfig {
                estimators: NativePriceEstimators::new(vec![vec![NativePriceEstimator::CoinGecko]]),
                fallback_estimators: None,
                ..NativePriceConfig::test_default()
            },
            order_quoting: TestDefault::test_default(),
            database: TestDefault::test_default(),
            http_client: Default::default(),
            price_estimation: Default::default(),
            order_simulation: Default::default(),
        };

        let serialized = toml::to_string_pretty(&config).unwrap();
        let deserialized: Configuration = toml::from_str(&serialized).unwrap();

        assert_eq!(config.app_data_size_limit, deserialized.app_data_size_limit);
        assert_eq!(
            config.active_order_competition_threshold,
            deserialized.active_order_competition_threshold
        );
        assert!(deserialized.volume_fee.is_some());
        assert_eq!(
            config.volume_fee.as_ref().unwrap().factor.unwrap().get(),
            deserialized
                .volume_fee
                .as_ref()
                .unwrap()
                .factor
                .unwrap()
                .get()
        );
        assert_eq!(config.unsupported_tokens, deserialized.unsupported_tokens);
        assert_eq!(
            config.banned_users.addresses,
            deserialized.banned_users.addresses
        );
        assert_eq!(
            config.eip1271_skip_creation_validation,
            deserialized.eip1271_skip_creation_validation
        );
        assert_eq!(
            config.hide_competition_before_deadline,
            deserialized.hide_competition_before_deadline
        );
        assert_eq!(config.http_client.timeout, deserialized.http_client.timeout)
    }
}
