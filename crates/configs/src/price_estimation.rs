use {
    alloy::primitives::Address,
    bigdecimal::BigDecimal,
    serde::Deserialize,
    std::{str::FromStr, time::Duration},
    url::Url,
};

/// Controls which level of quote verification gets applied.
#[derive(Copy, Clone, Debug, Default, serde::Deserialize)]
#[cfg_attr(any(test, feature = "test-util"), derive(serde::Serialize))]
#[serde(rename_all = "kebab-case")]
pub enum QuoteVerificationMode {
    /// Quotes do not get verified.
    #[default]
    Unverified,
    /// Quotes get verified whenever possible and verified
    /// quotes are preferred over unverified ones.
    Prefer,
    /// Quotes get discarded if they can't be verified.
    /// Some scenarios like missing sell token balance are exempt.
    EnforceWhenPossible,
}

const fn default_quote_timeout() -> Duration {
    Duration::from_secs(5)
}

fn default_quote_inaccuracy_limit() -> BigDecimal {
    BigDecimal::from(1)
}

#[derive(Deserialize)]
#[cfg_attr(any(test, feature = "test-util"), derive(serde::Serialize))]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct PriceEstimation {
    /// Tenderly configuration (URL, project & API key).
    #[serde(default)]
    pub tenderly: TenderlyConfig,

    /// Configures the back off strategy for price estimators when requests take
    /// too long. Requests issued while back off is active get dropped
    /// entirely.
    #[serde(default)]
    pub price_estimation_rate_limiter: Option<rate_limit::Strategy>,

    /// The amount in native token atoms to use for price estimation. Should be
    /// reasonably large so that small pools do not influence the prices. If
    /// not set, a reasonable default is used based on network id.
    #[serde(default)]
    pub amount_to_estimate_prices_with: Option<alloy::primitives::U256>,

    /// The CoinGecko native price configuration.
    #[serde(default)]
    pub coin_gecko: CoinGeckoConfig,

    /// How inaccurate a quote must be before it gets discarded, provided as a
    /// factor. E.g. a value of `0.01` means at most 1 percent of the sell or
    /// buy tokens can be paid out of the settlement contract buffers.
    #[serde(default = "default_quote_inaccuracy_limit")]
    pub quote_inaccuracy_limit: BigDecimal,

    /// How strict quote verification should be.
    #[serde(default)]
    pub quote_verification: QuoteVerificationMode,

    /// Default timeout for quote requests.
    #[serde(with = "humantime_serde", default = "default_quote_timeout")]
    pub quote_timeout: Duration,

    #[serde(default)]
    pub balance_overrides: BalanceOverridesConfig,

    /// Tokens for which quote verification should not be attempted. This is an
    /// escape hatch when there is a very bad but verifiable liquidity source
    /// that would win against a very good but unverifiable liquidity source
    /// (e.g. private liquidity that exists but can't be verified).
    #[serde(default)]
    pub tokens_without_verification: Vec<Address>,

    /// 1-inch API connection settings (URL & key).
    #[serde(default)]
    pub one_inch: OneInchApi,
}

impl Default for PriceEstimation {
    fn default() -> Self {
        Self {
            tenderly: Default::default(),
            price_estimation_rate_limiter: None,
            amount_to_estimate_prices_with: None,
            one_inch: Default::default(),
            coin_gecko: Default::default(),
            quote_inaccuracy_limit: default_quote_inaccuracy_limit(),
            quote_verification: Default::default(),
            quote_timeout: default_quote_timeout(),
            balance_overrides: Default::default(),
            tokens_without_verification: Default::default(),
        }
    }
}

#[cfg(any(test, feature = "test-util"))]
impl crate::test_util::TestDefault for PriceEstimation {
    fn test_default() -> Self {
        Self {
            amount_to_estimate_prices_with: Some(alloy::primitives::U256::from(
                1_000_000_000_000_000_000u64,
            )),
            quote_timeout: Duration::from_secs(10),
            quote_verification: QuoteVerificationMode::EnforceWhenPossible,
            ..Default::default()
        }
    }
}

impl std::fmt::Debug for PriceEstimation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PriceEstimation")
            .field("tenderly", &self.tenderly)
            .field(
                "price_estimation_rate_limiter",
                &self.price_estimation_rate_limiter,
            )
            .field(
                "amount_to_estimate_prices_with",
                &self.amount_to_estimate_prices_with,
            )
            .field("coin_gecko", &self.coin_gecko)
            .field("quote_inaccuracy_limit", &self.quote_inaccuracy_limit)
            .field("quote_verification", &self.quote_verification)
            .field("quote_timeout", &self.quote_timeout)
            .field("balance_overrides", &self.balance_overrides)
            .field(
                "tokens_without_verification",
                &self.tokens_without_verification,
            )
            .field("one_inch", &self.one_inch)
            .finish()
    }
}

#[derive(Default, Deserialize)]
#[cfg_attr(any(test, feature = "test-util"), derive(serde::Serialize))]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct TenderlyConfig {
    /// The Tenderly user associated with the API key.
    #[serde(default)]
    pub user: Option<String>,

    /// The Tenderly project associated with the API key.
    #[serde(default)]
    pub project: Option<String>,

    /// Tenderly requires an API key to work. Optional since Tenderly could be
    /// skipped in access lists estimators.
    #[serde(default)]
    pub api_key: Option<String>,
}

impl std::fmt::Debug for TenderlyConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TenderlyConfig")
            .field("user", &self.user)
            .field("project", &self.project)
            .field("api_key", &"<REDACTED>")
            .finish()
    }
}

fn default_coin_gecko_url() -> Url {
    Url::from_str("https://api.coingecko.com/api/v3/simple/token_price")
        .expect("url should be valid")
}

#[derive(Deserialize)]
#[cfg_attr(any(test, feature = "test-util"), derive(serde::Serialize))]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct CoinGeckoConfig {
    /// The API key for the CoinGecko API.
    #[serde(
        default,
        deserialize_with = "crate::deserialize_env::deserialize_optional_string_from_env"
    )]
    pub api_key: Option<String>,

    /// The base URL for the CoinGecko API.
    #[serde(default = "default_coin_gecko_url")]
    pub url: Url,

    #[serde(default)]
    pub buffered: Option<CoinGeckoBufferedConfig>,
}

impl Default for CoinGeckoConfig {
    fn default() -> Self {
        Self {
            api_key: None,
            url: default_coin_gecko_url(),
            buffered: None,
        }
    }
}

impl std::fmt::Debug for CoinGeckoConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CoinGeckoConfig")
            .field("api_key", &"<REDACTED>")
            .field("url", &self.url)
            .field("buffered", &self.buffered)
            .finish()
    }
}

#[derive(Debug, Deserialize)]
#[cfg_attr(any(test, feature = "test-util"), derive(serde::Serialize))]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct CoinGeckoBufferedConfig {
    /// An additional minimum delay to wait for collecting CoinGecko requests.
    ///
    /// The delay to start counting after receiving the first request.
    #[serde(with = "humantime_serde")]
    pub debouncing_time: Duration,

    /// Maximum capacity of the broadcast channel to store the CoinGecko native
    /// prices results
    pub broadcast_channel_capacity: usize,
}

const fn default_probing_depth() -> u8 {
    60
}

const fn default_cache_size() -> usize {
    1000
}

#[derive(Debug, Deserialize)]
#[cfg_attr(any(test, feature = "test-util"), derive(serde::Serialize))]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct BalanceOverridesConfig {
    /// Token configuration for simulated balances on verified quotes. This
    /// allows the quote verification system to produce verified quotes for
    /// traders without sufficient balance for the configured token pairs.
    #[serde(default)]
    pub token_overrides: balance_overrides::TokenConfiguration,

    /// Enable automatic detection of token balance overrides. Pre-configured
    /// values in `token_overrides` take precedence.
    #[serde(default)]
    pub autodetect: bool,

    /// Controls how many storage slots get probed per storage entry point
    /// for automatically detecting how to override the balances of a token.
    #[serde(default = "default_probing_depth")]
    pub probing_depth: u8,

    /// Controls for how many tokens we store the result of the automatic
    /// balance override detection before evicting less used entries.
    #[serde(default = "default_cache_size")]
    pub cache_size: usize,
}

impl Default for BalanceOverridesConfig {
    fn default() -> Self {
        Self {
            token_overrides: Default::default(),
            autodetect: false,
            probing_depth: default_probing_depth(),
            cache_size: default_cache_size(),
        }
    }
}

fn default_one_inch_url() -> Url {
    Url::from_str("https://api.1inch.dev/").expect("url should be valid")
}

#[derive(Deserialize)]
#[cfg_attr(any(test, feature = "test-util"), derive(serde::Serialize))]
pub struct OneInchApi {
    /// The base URL for the 1Inch API.
    #[serde(default = "default_one_inch_url")]
    pub url: Url,

    /// The API key for the 1Inch API.
    #[serde(default)]
    pub api_key: Option<String>,
}

impl Default for OneInchApi {
    fn default() -> Self {
        Self {
            url: default_one_inch_url(),
            api_key: Default::default(),
        }
    }
}

impl std::fmt::Debug for OneInchApi {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("OneInchApi")
            .field("url", &self.url)
            .field("api_key", &"<REDACTED>")
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deserialize_defaults() {
        let toml = "";
        let config: PriceEstimation = toml::from_str(toml).unwrap();
        assert!(config.tenderly.user.is_none());
        assert!(config.tenderly.project.is_none());
        assert!(config.tenderly.api_key.is_none());
        assert!(config.price_estimation_rate_limiter.is_none());
        assert!(config.amount_to_estimate_prices_with.is_none());
        assert!(config.one_inch.api_key.is_none());
        assert_eq!(config.one_inch.url.as_str(), "https://api.1inch.dev/");
        assert!(config.coin_gecko.api_key.is_none());
        assert_eq!(
            config.coin_gecko.url.as_str(),
            "https://api.coingecko.com/api/v3/simple/token_price"
        );
        assert!(config.coin_gecko.buffered.is_none());
        assert_eq!(config.quote_inaccuracy_limit, BigDecimal::from(1));
        assert!(matches!(
            config.quote_verification,
            QuoteVerificationMode::Unverified
        ));
        assert_eq!(config.quote_timeout, Duration::from_secs(5));
        assert!(!config.balance_overrides.autodetect);
        assert_eq!(config.balance_overrides.probing_depth, 60);
        assert_eq!(config.balance_overrides.cache_size, 1000);
        assert!(config.tokens_without_verification.is_empty());
    }

    #[test]
    fn deserialize_full() {
        let toml = r#"
        one-inch-api-key = "my-1inch-key"
        one-inch-url = "https://custom.1inch.dev/"
        quote-inaccuracy-limit = "0.01"
        quote-verification = "enforce-when-possible"
        quote-timeout = "10s"
        tokens-without-verification = ["0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2"]
        amount-to-estimate-prices-with = "1000000000000000000"

        [price-estimation-rate-limiter]
        back-off-growth-factor = 2.0
        min-back-off = "1s"
        max-back-off = "30s"

        [tenderly]
        user = "my-user"
        project = "my-project"
        api-key = "my-tenderly-key"

        [coin-gecko]
        api-key = "my-cg-key"
        url = "https://pro-api.coingecko.com/api/v3/simple/token_price"

        [coin-gecko.buffered]
        debouncing-time = "500ms"
        broadcast-channel-capacity = 100

        [balance-overrides]
        autodetect = true
        probing-depth = 30
        cache-size = 500
        "#;
        let config: PriceEstimation = toml::from_str(toml).unwrap();

        assert_eq!(config.tenderly.user.as_deref(), Some("my-user"));
        assert_eq!(config.tenderly.project.as_deref(), Some("my-project"));
        assert_eq!(config.tenderly.api_key.as_deref(), Some("my-tenderly-key"));
        assert!(config.price_estimation_rate_limiter.is_some());
        assert_eq!(
            config.amount_to_estimate_prices_with,
            Some(alloy::primitives::U256::from(1_000_000_000_000_000_000u64))
        );
        assert_eq!(config.one_inch.api_key.as_deref(), Some("my-1inch-key"));
        assert_eq!(config.one_inch.url.as_str(), "https://custom.1inch.dev/");
        assert_eq!(config.coin_gecko.api_key.as_deref(), Some("my-cg-key"));
        assert_eq!(
            config.coin_gecko.url.as_str(),
            "https://pro-api.coingecko.com/api/v3/simple/token_price"
        );
        let buffered = config.coin_gecko.buffered.as_ref().unwrap();
        assert_eq!(buffered.debouncing_time, Duration::from_millis(500));
        assert_eq!(buffered.broadcast_channel_capacity, 100);
        assert_eq!(config.quote_inaccuracy_limit.to_string(), "0.01");
        assert!(matches!(
            config.quote_verification,
            QuoteVerificationMode::EnforceWhenPossible
        ));
        assert_eq!(config.quote_timeout, Duration::from_secs(10));
        assert!(config.balance_overrides.autodetect);
        assert_eq!(config.balance_overrides.probing_depth, 30);
        assert_eq!(config.balance_overrides.cache_size, 500);
        assert_eq!(config.tokens_without_verification.len(), 1);
    }

    #[test]
    fn roundtrip_serialization() {
        let config = PriceEstimation {
            quote_timeout: Duration::from_secs(10),
            quote_verification: QuoteVerificationMode::Prefer,
            ..Default::default()
        };
        let serialized = toml::to_string_pretty(&config).unwrap();
        let deserialized: PriceEstimation = toml::from_str(&serialized).unwrap();
        assert_eq!(config.quote_timeout, deserialized.quote_timeout);
        assert!(matches!(
            deserialized.quote_verification,
            QuoteVerificationMode::Prefer
        ));
    }

    #[test]
    fn debug_redacts_secrets() {
        let config = PriceEstimation {
            one_inch: OneInchApi {
                api_key: Some("secret".to_string()),
                ..Default::default()
            },
            coin_gecko: CoinGeckoConfig {
                api_key: Some("secret".to_string()),
                ..Default::default()
            },
            tenderly: TenderlyConfig {
                api_key: Some("secret".to_string()),
                ..Default::default()
            },
            ..Default::default()
        };
        let debug = format!("{config:?}");
        assert!(!debug.contains("secret"));
        assert!(debug.contains("<REDACTED>"));
    }
}
