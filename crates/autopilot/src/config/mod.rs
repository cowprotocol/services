use {
    crate::config::{fee_policy::FeePoliciesConfig, solver::Solver},
    anyhow::{anyhow, ensure},
    serde::{Deserialize, Serialize},
    std::path::Path,
};

pub mod fee_policy;
pub mod solver;

#[derive(Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct Configuration {
    pub drivers: Vec<Solver>,

    /// Describes how the protocol fees should be calculated.
    pub fee_policies: FeePoliciesConfig,
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
    }

    #[test]
    fn deserialize_configuration_defaults() {
        let toml = r#"
        [[drivers]]
        name = "solver1"
        url = "http://localhost:8080"
        submission-account.address = "0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2"

        [fee-policies]
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
        };

        let serialized = toml::to_string_pretty(&config).unwrap();
        let deserialized: Configuration = toml::from_str(&serialized).unwrap();

        assert_eq!(deserialized.drivers.len(), 1);
        assert_eq!(deserialized.fee_policies.policies.len(), 1);
        assert_eq!(deserialized.fee_policies.max_partner_fee.get(), 0.02);
    }
}
