use {
    alloy::primitives::Address,
    anyhow::{Context, Result, ensure},
    itertools::Itertools,
    serde::{Deserialize, Serialize},
    std::{
        fmt::{self, Display, Formatter},
        num::NonZeroUsize,
        str::FromStr,
        time::Duration,
    },
    url::Url,
};

#[derive(Clone, Debug, Default, Serialize)]
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
        match estimators
            .iter()
            .enumerate()
            .find_map(|(n, stage)| stage.is_empty().then_some(n))
        {
            Some(n) => Err(serde::de::Error::invalid_length(
                0,
                &format!("stage {} is empty, all stages must not be empty", n).as_str(),
            )),
            None => Ok(Self(estimators)),
        }
    }
}

impl NativePriceEstimators {
    pub fn new(estimators: Vec<Vec<NativePriceEstimator>>) -> Self {
        Self(estimators)
    }
}

#[cfg(any(test, feature = "test-util"))]
impl NativePriceEstimators {
    /// Returns a list with a single stage, said stage contains a Driver estimator named `test_quoter` with URL `http://localhost:11088/test_solver`.
    pub fn test_default() -> Self {
        NativePriceEstimators::new(vec![vec![NativePriceEstimator::driver(
            "test_quoter".to_string(),
            Url::from_str("http://localhost:11088/test_solver").unwrap(),
        )]])
    }
}

#[derive(Clone, Debug, Hash, Eq, PartialEq, Deserialize, Serialize)]
#[serde(tag = "type")]
pub enum NativePriceEstimator {
    Driver(ExternalSolver),
    Forwarder { url: Url },
    OneInchSpotPriceApi,
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
        let formatter = match self {
            NativePriceEstimator::Driver(s) => format!("Driver|{}|{}", &s.name, s.url),
            NativePriceEstimator::Forwarder { url } => format!("Forwarder|{}", url),
            NativePriceEstimator::OneInchSpotPriceApi => "OneInchSpotPriceApi".into(),
            NativePriceEstimator::CoinGecko => "CoinGecko".into(),
        };
        write!(f, "{formatter}")
    }
}

impl NativePriceEstimators {
    pub fn as_slice(&self) -> &[Vec<NativePriceEstimator>] {
        &self.0
    }
}

impl Display for NativePriceEstimators {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        let formatter = self
            .as_slice()
            .iter()
            .map(|stage| {
                stage
                    .iter()
                    .format_with(",", |estimator, f| f(&format_args!("{estimator}")))
            })
            .format(";");
        write!(f, "{formatter}")
    }
}

impl FromStr for NativePriceEstimators {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Self(
            s.split(';')
                .map(|sub_list| {
                    sub_list
                        .split(',')
                        .map(NativePriceEstimator::from_str)
                        .collect::<Result<Vec<NativePriceEstimator>>>()
                })
                .collect::<Result<Vec<Vec<NativePriceEstimator>>>>()?,
        ))
    }
}

impl FromStr for NativePriceEstimator {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let (variant, args) = s.split_once('|').unwrap_or((s, ""));
        match variant {
            "OneInchSpotPriceApi" => Ok(NativePriceEstimator::OneInchSpotPriceApi),
            "CoinGecko" => Ok(NativePriceEstimator::CoinGecko),
            "Driver" => Ok(NativePriceEstimator::Driver(ExternalSolver::from_str(
                args,
            )?)),
            "Forwarder" => Ok(NativePriceEstimator::Forwarder {
                url: args
                    .parse()
                    .context("Forwarder price estimator invalid URL")?,
            }),
            _ => Err(anyhow::anyhow!("unsupported native price estimator: {}", s)),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize, Serialize)]
pub struct ExternalSolver {
    pub name: String,
    pub url: Url,
}

impl FromStr for ExternalSolver {
    type Err = anyhow::Error;

    fn from_str(solver: &str) -> Result<Self> {
        let parts: Vec<&str> = solver.split('|').collect();
        ensure!(parts.len() >= 2, "not enough arguments for external solver");
        let (name, url) = (parts[0], parts[1]);
        let url: Url = url.parse()?;
        Ok(Self {
            name: name.to_owned(),
            url,
        })
    }
}

const fn default_cache_max_age() -> Duration {
    Duration::from_mins(10)
}

const fn default_cache_concurrent_requests() -> NonZeroUsize {
    NonZeroUsize::new(1).expect("value should be greater than 0")
}

const fn default_results_required() -> NonZeroUsize {
    NonZeroUsize::new(2).expect("value should not be zero")
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct NativePriceConfig {
    /// List of mappings of native price tokens substitutions with approximated:
    /// - the first is a token address for which we get the native token price
    /// - the second is a token address used for the price approximation
    #[serde(default)]
    pub approximation_tokens: Vec<(Address, Address)>,

    /// Configuration for the native price caching mechanism.
    #[serde(default)]
    pub cache: CacheConfig,

    /// How many successful price estimates for each order will cause a native
    /// price estimation to return its result early.
    ///
    /// As this value increases, the fast estimator behavior will approximate
    /// the behavior of the optimal estimator.
    ///
    /// It's possible to pass values greater than the total number of enabled
    /// estimators but that will not have any further effect.
    #[serde(default = "default_results_required")]
    pub results_required: NonZeroUsize,
}

impl Default for NativePriceConfig {
    fn default() -> Self {
        Self {
            approximation_tokens: Default::default(),
            cache: Default::default(),
            results_required: default_results_required(),
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub struct CacheConfig {
    /// How long cached native prices stay valid.
    #[serde(default = "default_cache_max_age", with = "humantime_serde")]
    pub max_age: Duration,

    /// How many price estimation requests can be executed concurrently in the
    /// maintenance task.
    #[serde(default = "default_cache_concurrent_requests")]
    pub concurrent_requests: NonZeroUsize,
}

impl Default for CacheConfig {
    fn default() -> Self {
        Self {
            max_age: default_cache_max_age(),
            concurrent_requests: default_cache_concurrent_requests(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deserialize_full() {
        let toml = r#"
            approximation-tokens = [
                ["0x0000000000000000000000000000000000000001", "0x0000000000000000000000000000000000000002"],
            ]

            [cache]
            max-age = "5m"
            concurrent-requests = 4
        "#;
        let config: NativePriceConfig = toml::from_str(toml).unwrap();
        assert_eq!(config.approximation_tokens.len(), 1);
        assert_eq!(config.cache.max_age, Duration::from_secs(300));
        assert_eq!(
            config.cache.concurrent_requests,
            NonZeroUsize::new(4).unwrap()
        );
    }

    #[test]
    fn cache_defaults() {
        let toml = r#"
            approximation-tokens = []
            [cache]
        "#;
        let config: NativePriceConfig = toml::from_str(toml).unwrap();
        assert_eq!(config.cache.max_age, Duration::from_mins(10));
        assert_eq!(
            config.cache.concurrent_requests,
            NonZeroUsize::new(1).unwrap()
        );
    }

    #[test]
    fn multiple_approximation_tokens() {
        let toml = r#"
            approximation-tokens = [
                ["0x0000000000000000000000000000000000000001", "0x0000000000000000000000000000000000000002"],
                ["0x0000000000000000000000000000000000000003", "0x0000000000000000000000000000000000000004"],
            ]
            [cache]
        "#;
        let config: NativePriceConfig = toml::from_str(toml).unwrap();
        assert_eq!(config.approximation_tokens.len(), 2);
        assert_eq!(
            config.approximation_tokens[0].0,
            Address::from_slice(&[0; 19].into_iter().chain([1]).collect::<Vec<_>>()),
        );
    }

    #[test]
    fn roundtrip_serialization() {
        let config = NativePriceConfig {
            approximation_tokens: vec![(Address::repeat_byte(1), Address::repeat_byte(2))],
            cache: CacheConfig {
                max_age: Duration::from_secs(120),
                concurrent_requests: NonZeroUsize::new(8).unwrap(),
            },
            results_required: default_results_required(),
        };

        let serialized = toml::to_string_pretty(&config).unwrap();
        let deserialized: NativePriceConfig = toml::from_str(&serialized).unwrap();

        assert_eq!(
            config.approximation_tokens,
            deserialized.approximation_tokens,
        );
        assert_eq!(config.cache.max_age, deserialized.cache.max_age);
        assert_eq!(
            config.cache.concurrent_requests,
            deserialized.cache.concurrent_requests,
        );
    }
}
