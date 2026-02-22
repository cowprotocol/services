use {
    crate::config::{
        banned_users::BannedUsersConfig,
        fee_policy::FeePoliciesConfig,
        native_price::NativePriceConfig,
        order_events_cleanup::OrderEventsCleanupConfig,
        s3::S3Config,
        solver::Solver,
        trusted_tokens::TrustedTokensConfig,
    },
    anyhow::{anyhow, ensure},
    serde::{Deserialize, Serialize},
    std::path::Path,
};

pub mod banned_users;
pub mod fee_policy;
pub mod native_price;
pub mod order_events_cleanup;
pub mod s3;
pub mod solver;
pub mod trusted_tokens;

#[derive(Debug, Default, Deserialize, Serialize)]
// NOTE: cannot add deny_unknown_fields during the config migration
// as new ones get added in the config will fail parsing if extra fields are present
#[serde(rename_all = "kebab-case", /* deny_unknown_fields */)]
pub struct Configuration {
    pub drivers: Vec<Solver>,

    /// Describes how the protocol fees should be calculated.
    pub fee_policies: FeePoliciesConfig,

    /// Configuration for trusted tokens that the settlement contract is willing
    /// to internalize.
    #[serde(default)]
    pub trusted_tokens: TrustedTokensConfig,

    /// Configuration for periodic cleanup of order events.
    #[serde(default)]
    pub order_events_cleanup: OrderEventsCleanupConfig,

    /// Configuration for order validation rules.
    #[serde(default)]
    pub banned_users: BannedUsersConfig,

    /// Configuration for uploading auction instances to S3.
    /// If absent, S3 uploads are disabled.
    #[serde(default)]
    pub s3: Option<S3Config>,

    pub native_price_estimation: NativePriceConfig,
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

    pub async fn to_path<P: AsRef<Path>>(&self, path: P) -> anyhow::Result<()> {
        Ok(tokio::fs::write(path, toml::to_string_pretty(self)?).await?)
    }

    // Note for reviewers: if this and other validations are always applied,
    // we should instead move them to the deserialization stage
    // https://lexi-lambda.github.io/blog/2019/11/05/parse-don-t-validate/
    pub fn validate(self) -> anyhow::Result<Self> {
        ensure!(
            !self.drivers.is_empty(),
            "colocation is enabled but no drivers are configured"
        );
        Ok(self)
    }
}

#[cfg(any(test, feature = "test-util"))]
impl Configuration {
    pub fn test(name: &str, solver_address: alloy::primitives::Address) -> Self {
        Self {
            drivers: vec![Solver::test(name, solver_address)],
            ..Default::default()
        }
    }

    pub fn to_temp_path(&self) -> tempfile::NamedTempFile {
        use std::io::Write;
        let mut file = tempfile::NamedTempFile::new().expect("temp file creation should not fail");
        file.write_all(
            toml::to_string_pretty(self)
                .expect("serialization should not fail")
                .as_bytes(),
        )
        .expect("writing to temp file should not fail");
        file
    }

    pub fn to_cli_args(&self) -> (tempfile::NamedTempFile, String) {
        // Must return the temp_file because it gets deleted on drop
        // disabling the cleanup will lead to a bunch of artifacts laying around
        let named_temp_file = self.to_temp_path();
        let cli_arg = format!("--config={}", named_temp_file.path().display());
        (named_temp_file, cli_arg)
    }
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        crate::config::{
            fee_policy::{FeePolicy, FeePolicyKind, FeePolicyOrderClass, UpcomingFeePolicies},
            solver::Account,
        },
        alloy::primitives::address,
        std::time::Duration,
    };

    #[test]
    fn deserialize_full_configuration() {
        let toml = r#"
        [[drivers]]
        name = "solver1"
        url = "http://localhost:8080"
        submission-account.address = "0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2"

        [[drivers]]
        name = "solver2"
        url = "http://localhost:8081"
        submission-account.kms = "arn:aws:kms:us-east-1:123456789012:key/12345678-1234-1234-1234-123456789012"

        [fee-policies]
        max-partner-fee = 0.005

        [[fee-policies.policies]]
        kind.surplus = { factor = 0.5, max-volume-factor = 0.9 }
        order-class = "limit"

        [[fee-policies.policies]]
        kind.volume = { factor = 0.1 }
        order-class = "any"

        [fee-policies.upcoming-policies]
        effective-from-timestamp = "2025-06-01T00:00:00Z"

        [[fee-policies.upcoming-policies.policies]]
        kind.volume = { factor = 0.2 }
        order-class = "any"

        [trusted-tokens]
        url = "https://example.com/tokens.json"
        tokens = ["0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2"]
        update-interval = "30m"

        [order-events-cleanup]
        cleanup-interval = "12h"
        cleanup-threshold = "7d"

        [banned-users]
        addresses = ["0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2"]
        max-cache-size = 5000

        [s3]
        bucket = "my-bucket"
        filename-prefix = "staging/mainnet/"

        [native-price-estimation]
        estimators = [["CoinGecko"]]
        "#;

        let config: Configuration = toml::from_str(toml).unwrap();

        assert_eq!(config.drivers.len(), 2);
        assert_eq!(config.drivers[0].name, "solver1");
        assert_eq!(
            config.drivers[0].submission_account,
            Account::Address(address!("C02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2"))
        );
        assert_eq!(config.drivers[1].name, "solver2");

        assert_eq!(config.fee_policies.max_partner_fee.get(), 0.005);
        assert_eq!(config.fee_policies.policies.len(), 2);
        assert!(matches!(
            config.fee_policies.policies[0].kind,
            FeePolicyKind::Surplus { .. }
        ));
        assert!(matches!(
            config.fee_policies.policies[0].order_class,
            FeePolicyOrderClass::Limit
        ));
        assert!(matches!(
            config.fee_policies.policies[1].kind,
            FeePolicyKind::Volume { .. }
        ));
        assert!(matches!(
            config.fee_policies.policies[1].order_class,
            FeePolicyOrderClass::Any
        ));

        assert_eq!(config.fee_policies.upcoming_policies.policies.len(), 1);
        assert!(
            config
                .fee_policies
                .upcoming_policies
                .effective_from_timestamp
                .is_some()
        );

        assert!(config.trusted_tokens.url.is_some());
        assert_eq!(config.trusted_tokens.tokens.len(), 1);
        assert_eq!(
            config.trusted_tokens.update_interval,
            Duration::from_secs(1800)
        );

        assert_eq!(
            config.order_events_cleanup.cleanup_interval,
            Duration::from_secs(43200)
        );
        assert_eq!(
            config.order_events_cleanup.cleanup_threshold,
            Duration::from_secs(604800)
        );

        assert_eq!(config.banned_users.addresses.len(), 1);
        assert_eq!(config.banned_users.max_cache_size.get(), 5000);

        let s3 = config.s3.unwrap();
        assert_eq!(s3.bucket, "my-bucket");
        assert_eq!(s3.filename_prefix, "staging/mainnet/");
    }

    #[test]
    fn deserialize_configuration_defaults() {
        let toml = r#"
        [[drivers]]
        name = "solver1"
        url = "http://localhost:8080"
        submission-account.address = "0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2"

        [fee-policies]

        [native-price-estimation]
        estimators = []
        "#;

        let config: Configuration = toml::from_str(toml).unwrap();

        assert_eq!(config.drivers.len(), 1);
        assert!(config.fee_policies.policies.is_empty());
        assert_eq!(config.fee_policies.max_partner_fee.get(), 0.01);
        assert!(config.fee_policies.upcoming_policies.policies.is_empty());
        assert!(
            config
                .fee_policies
                .upcoming_policies
                .effective_from_timestamp
                .is_none()
        );

        assert!(config.trusted_tokens.url.is_none());
        assert!(config.trusted_tokens.tokens.is_empty());
        assert_eq!(
            config.trusted_tokens.update_interval,
            Duration::from_secs(3600)
        );

        assert_eq!(
            config.order_events_cleanup.cleanup_interval,
            Duration::from_secs(86400)
        );
        assert_eq!(
            config.order_events_cleanup.cleanup_threshold,
            Duration::from_secs(2592000)
        );

        assert!(config.banned_users.addresses.is_empty());
        assert_eq!(config.banned_users.max_cache_size.get(), 10000);

        assert!(config.s3.is_none());
    }

    #[test]
    fn roundtrip_serialization() {
        let config = Configuration {
            drivers: vec![Solver::test(
                "solver1",
                address!("C02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2"),
            )],
            fee_policies: FeePoliciesConfig {
                policies: vec![FeePolicy {
                    kind: FeePolicyKind::Surplus {
                        factor: 0.5.try_into().unwrap(),
                        max_volume_factor: 0.9.try_into().unwrap(),
                    },
                    order_class: FeePolicyOrderClass::Limit,
                }],
                max_partner_fee: 0.02.try_into().unwrap(),
                upcoming_policies: UpcomingFeePolicies::default(),
            },
            ..Default::default()
        };

        let serialized = toml::to_string_pretty(&config).unwrap();
        let deserialized: Configuration = toml::from_str(&serialized).unwrap();

        assert_eq!(deserialized.drivers.len(), 1);
        assert_eq!(deserialized.fee_policies.policies.len(), 1);
        assert_eq!(deserialized.fee_policies.max_partner_fee.get(), 0.02);
    }
}
