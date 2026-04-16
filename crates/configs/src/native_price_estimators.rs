use {
    serde::Deserialize,
    std::fmt::{self, Display, Formatter},
    url::Url,
};

/// Ordered stages of native-price estimators. Each stage is tried in order;
/// within a stage estimators run concurrently.
#[derive(Clone, Debug, Default)]
#[cfg_attr(any(test, feature = "test-util"), derive(serde::Serialize))]
pub struct NativePriceEstimators(Vec<Vec<NativePriceEstimator>>);

impl<'de> Deserialize<'de> for NativePriceEstimators {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let estimators = <Vec<Vec<NativePriceEstimator>>>::deserialize(deserializer)?;
        if estimators.is_empty() {
            return Err(serde::de::Error::invalid_length(
                0,
                &"expected native price estimator stages to be configured",
            ));
        }
        for (n, stage) in estimators.iter().enumerate() {
            if stage.is_empty() {
                return Err(serde::de::Error::invalid_length(
                    0,
                    &format!("stage {} is empty, all stages must not be empty", n).as_str(),
                ));
            }
        }
        Ok(Self(estimators))
    }
}

impl NativePriceEstimators {
    pub fn new(estimators: Vec<Vec<NativePriceEstimator>>) -> Self {
        Self(estimators)
    }

    pub fn as_slice(&self) -> &[Vec<NativePriceEstimator>] {
        &self.0
    }
}

#[cfg(any(test, feature = "test-util"))]
impl NativePriceEstimators {
    pub fn test_default() -> Self {
        use std::str::FromStr;
        NativePriceEstimators::new(vec![vec![NativePriceEstimator::driver(
            "test_quoter".to_string(),
            Url::from_str("http://localhost:11088/test_solver").unwrap(),
        )]])
    }
}

impl Display for NativePriceEstimators {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        for (i, stage) in self.as_slice().iter().enumerate() {
            if i > 0 {
                write!(f, ";")?;
            }
            for (j, estimator) in stage.iter().enumerate() {
                if j > 0 {
                    write!(f, ",")?;
                }
                write!(f, "{estimator}")?;
            }
        }
        Ok(())
    }
}

/// Reference to an external solver by name and URL.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize)]
#[cfg_attr(any(test, feature = "test-util"), derive(serde::Serialize))]
#[serde(deny_unknown_fields)]
pub struct ExternalSolver {
    pub name: String,
    pub url: Url,
}

/// A single native-price estimation backend.
#[derive(Clone, Debug, Hash, Eq, PartialEq, Deserialize)]
#[cfg_attr(any(test, feature = "test-util"), derive(serde::Serialize))]
#[serde(tag = "type")]
pub enum NativePriceEstimator {
    /// Query an external solver driver for native prices.
    Driver(ExternalSolver),
    /// Forward requests to another service (e.g. autopilot).
    Forwarder { url: Url },
    /// Use the 1inch spot-price API.
    OneInchSpotPriceApi,
    /// Use the CoinGecko API.
    CoinGecko,
}

impl NativePriceEstimator {
    pub const fn driver(name: String, url: Url) -> Self {
        Self::Driver(ExternalSolver { name, url })
    }

    pub const fn forwarder(url: Url) -> Self {
        Self::Forwarder { url }
    }
}

impl Display for NativePriceEstimator {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        match self {
            NativePriceEstimator::Driver(s) => write!(f, "Driver|{}|{}", &s.name, s.url),
            NativePriceEstimator::Forwarder { url } => write!(f, "Forwarder|{}", url),
            NativePriceEstimator::OneInchSpotPriceApi => write!(f, "OneInchSpotPriceApi"),
            NativePriceEstimator::CoinGecko => write!(f, "CoinGecko"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug, Deserialize)]
    struct Helper {
        estimators: NativePriceEstimators,
    }

    #[test]
    fn toml_deserialize_estimators_empty() {
        #[derive(Deserialize)]
        struct H {
            _estimators: NativePriceEstimators,
        }

        assert!(toml::from_str::<H>("estimators = []").is_err());
        assert!(toml::from_str::<H>("estimators = [[]]").is_err());
    }

    #[test]
    fn toml_deserialize_estimators_single_stage() {
        let toml = r#"
        estimators = [[{type = "CoinGecko"}, {type = "OneInchSpotPriceApi"}]]
        "#;

        let parsed: Helper = toml::from_str(toml).unwrap();
        assert_eq!(
            parsed.estimators.as_slice(),
            vec![vec![
                NativePriceEstimator::CoinGecko,
                NativePriceEstimator::OneInchSpotPriceApi,
            ]]
        );
    }

    #[test]
    fn toml_deserialize_estimators_multiple_stages() {
        let toml = r#"
        estimators = [
            [{type = "CoinGecko"}, {type = "Driver", name = "solver1", url = "http://localhost:8080"}],
            [{type = "Forwarder", url = "http://localhost:12088"}],
        ]
        "#;

        let parsed: Helper = toml::from_str(toml).unwrap();
        assert_eq!(
            parsed.estimators.as_slice(),
            vec![
                vec![
                    NativePriceEstimator::CoinGecko,
                    NativePriceEstimator::Driver(ExternalSolver {
                        name: "solver1".to_string(),
                        url: "http://localhost:8080".parse().unwrap(),
                    }),
                ],
                vec![NativePriceEstimator::Forwarder {
                    url: "http://localhost:12088".parse().unwrap(),
                }],
            ]
        );
    }

    #[test]
    fn toml_deserialize_estimators_default() {
        let estimators = NativePriceEstimators::default();
        assert!(estimators.as_slice().is_empty());
    }
}
