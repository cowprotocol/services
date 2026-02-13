use {
    alloy::primitives::Address,
    core::fmt,
    serde::{Deserialize, Deserializer, Serialize},
    std::fmt::{Display, Formatter},
    url::Url,
};

/// External solver driver configuration
#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub struct Solver {
    pub name: String,
    pub url: Url,
    pub submission_account: Account,
}

impl Solver {
    pub fn new(name: String, url: Url, account: Account) -> Self {
        Self {
            name,
            url,
            submission_account: account,
        }
    }
}

impl Display for Solver {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "{}({})", self.name, self.url)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Account {
    /// AWS KMS is used to retrieve the solver public key
    #[serde(deserialize_with = "deserialize_arn")]
    Kms(Arn),
    /// Solver public key
    Address(Address),
}

// Wrapper type for AWS ARN identifiers
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize)]
pub struct Arn(pub String);

fn deserialize_arn<'de, D>(deserializer: D) -> Result<Arn, D::Error>
where
    D: Deserializer<'de>,
{
    let raw_arn = String::deserialize(deserializer)?;
    if raw_arn.starts_with("arn:aws:kms:") {
        Ok(Arn(raw_arn))
    } else {
        Err(serde::de::Error::invalid_value(
            serde::de::Unexpected::Str(raw_arn.as_str()),
            &"expected value starting with \"arn:aws:kms\"",
        ))
    }
}

#[cfg(test)]
mod test {
    use {super::*, alloy::primitives::address};

    #[test]
    fn parse_driver_submission_account_address() {
        let toml = r#"
        name = "name1"
        url = "http://localhost:8080"
        submission-account.address = "0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2"
        "#;
        let driver = toml::from_str::<Solver>(toml).unwrap();

        let expected = Solver::new(
            "name1".into(),
            Url::parse("http://localhost:8080").unwrap(),
            Account::Address(address!("C02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2")),
        );
        assert_eq!(driver, expected);
    }

    #[test]
    fn parse_driver_submission_account_arn() {
        let toml = r#"
        name = "name1"
        url = "http://localhost:8080"
        submission-account.kms = "arn:aws:kms:supersecretstuff"
        "#;
        let driver = toml::from_str::<Solver>(toml).unwrap();

        let expected = Solver::new(
            "name1".into(),
            Url::parse("http://localhost:8080").unwrap(),
            Account::Kms(Arn("arn:aws:kms:supersecretstuff".into())),
        );
        assert_eq!(driver, expected);
    }

    #[test]
    fn parse_driver_with_threshold() {
        let toml = r#"
        name = "name1"
        url = "http://localhost:8080"
        submission-account.address = "0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2"
        fairness-threshold = "1000000000000000000"
        "#;
        let driver = toml::from_str::<Solver>(toml).unwrap();

        let expected = Solver::new(
            "name1".into(),
            Url::parse("http://localhost:8080").unwrap(),
            Account::Address(address!("C02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2")),
        );
        assert_eq!(driver, expected);
    }

    #[test]
    fn deserialize_valid_arn() {
        let toml = r#"kms = "arn:aws:kms:us-east-1:123456789012:key/12345678-1234-1234-1234-123456789012""#;
        let account = toml::from_str::<Account>(toml).unwrap();

        let expected = Account::Kms(Arn("arn:aws:kms:us-east-1:123456789012:key/\
                                         12345678-1234-1234-1234-123456789012"
            .into()));
        assert_eq!(account, expected);
    }

    #[test]
    fn deserialize_invalid_arn_wrong_prefix() {
        let toml = r#"kms = "arn:aws:s3:us-east-1:123456789012:bucket/mybucket""#;
        let result = toml::from_str::<Account>(toml);

        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(
            err.to_string()
                .contains("expected value starting with \"arn:aws:kms\""),
            "Error message: {}",
            err
        );
    }

    #[test]
    fn deserialize_invalid_arn_not_arn() {
        let toml = r#"kms = "not-an-arn""#;
        let result = toml::from_str::<Account>(toml);

        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(
            err.to_string()
                .contains("expected value starting with \"arn:aws:kms\""),
            "Error message: {}",
            err
        );
    }

    #[test]
    fn parse_multiple_solvers() {
        let toml = r#"
        [[drivers]]
        name = "solver1"
        url = "http://localhost:8080"
        submission-account.address = "0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2"

        [[drivers]]
        name = "solver2"
        url = "http://localhost:8081"
        fairness-threshold = "2000000000000000000"
        # test the format used in the infra repo
        [drivers.submission-account]
        kms = "arn:aws:kms:us-east-1:123456789012:key/12345678-1234-1234-1234-123456789012"
        "#;

        #[derive(Deserialize)]
        struct Config {
            drivers: Vec<Solver>,
        }

        let config = toml::from_str::<Config>(toml).unwrap();

        assert_eq!(config.drivers.len(), 2);

        let expected_solver1 = Solver::new(
            "solver1".into(),
            Url::parse("http://localhost:8080").unwrap(),
            Account::Address(address!("C02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2")),
        );

        let expected_solver2 = Solver::new(
            "solver2".into(),
            Url::parse("http://localhost:8081").unwrap(),
            Account::Kms(Arn("arn:aws:kms:us-east-1:123456789012:key/\
                              12345678-1234-1234-1234-123456789012"
                .into())),
        );

        assert_eq!(config.drivers[0], expected_solver1);
        assert_eq!(config.drivers[1], expected_solver2);
    }
}
