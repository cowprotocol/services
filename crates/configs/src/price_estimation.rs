use {
    crate::{balance_overrides, rate_limit::Strategy},
    alloy::primitives::{Address, U256, map::HashSet},
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

const fn default_max_gas_per_tx() -> u64 {
    16777215
}

#[derive(Deserialize, Debug)]
#[cfg_attr(any(test, feature = "test-util"), derive(serde::Serialize))]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct PriceEstimation {
    /// Configures the back off strategy for price estimators when requests take
    /// too long. Requests issued while back off is active get dropped
    /// entirely.
    pub price_estimation_rate_limiter: Option<Strategy>,

    /// The amount in native token atoms to use for price estimation. Should be
    /// reasonably large so that small pools do not influence the prices. If
    /// not set, a reasonable default is used based on network id.
    pub amount_to_estimate_prices_with: Option<U256>,

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
    pub tokens_without_verification: HashSet<Address>,

    /// How much gas a single tx may consume at most. Any quote using more than
    /// this will fail during the verification.
    /// Defaults to the maximum transaction gas limit Ethereum introduced in the
    /// Fusaka hardfork.
    #[serde(default = "default_max_gas_per_tx")]
    pub max_gas_per_tx: u64,

    /// Minimum gas amount for unverified quotes. When an unverified quote
    /// reports less gas than this, the floor is used instead. Verified quotes
    /// are unaffected. Defaults to 0 (disabled).
    ///
    /// Some tokens (e.g. Ondo RWA tokens) have high transfer costs that
    /// solvers underestimate in unverified quotes, leading to fees that don't
    /// cover execution gas and causing small orders to expire unfilled.
    #[serde(default)]
    pub min_gas_amount_for_unverified_quotes: u64,

    /// Maximum gas amount for unverified quotes. When an unverified quote
    /// reports more gas than this, the ceiling is used instead. Verified
    /// quotes are unaffected. Defaults to u64::MAX (disabled).
    ///
    /// This is a hack to alleviate tsolver issues where they report extremely
    /// high gas for RWA tokens.
    #[serde(default = "default_max_gas_amount_for_unverified_quotes")]
    pub max_gas_amount_for_unverified_quotes: u64,

    /// Tenderly configuration (URL, project & API key).
    #[serde(default)]
    pub tenderly: Option<crate::simulator::TenderlyConfig>,

    /// The CoinGecko native price configuration.
    pub coin_gecko: Option<CoinGeckoConfig>,

    /// 1-inch API connection settings (URL & key).
    pub one_inch: Option<OneInchApi>,
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
            max_gas_per_tx: default_max_gas_per_tx(),
            min_gas_amount_for_unverified_quotes: 0,
            max_gas_amount_for_unverified_quotes: 0,
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
        deserialize_with = "crate::deserialize_env::deserialize_string_from_env"
    )]
    pub api_key: String,

    /// The base URL for the CoinGecko API.
    #[serde(default = "default_coin_gecko_url")]
    pub url: Url,

    pub buffered: Option<CoinGeckoBufferedConfig>,
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

#[cfg(any(test, feature = "test-util"))]
impl crate::test_util::TestDefault for CoinGeckoConfig {
    fn test_default() -> Self {
        Self {
            api_key: "test-api-key".to_string(),
            url: default_coin_gecko_url(),
            buffered: None,
        }
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

fn default_max_gas_amount_for_unverified_quotes() -> u64 {
    u64::MAX
}

#[derive(Deserialize)]
#[cfg_attr(any(test, feature = "test-util"), derive(serde::Serialize))]
#[serde(rename_all = "kebab-case")]
pub struct OneInchApi {
    /// The base URL for the 1Inch API.
    #[serde(default = "default_one_inch_url")]
    pub url: Url,

    /// The API key for the 1Inch API.
    #[serde(default)]
    pub api_key: String,
}

impl std::fmt::Debug for OneInchApi {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("OneInchApi")
            .field("url", &self.url)
            .field("api_key", &"<REDACTED>")
            .finish()
    }
}

#[cfg(any(test, feature = "test-util"))]
impl crate::test_util::TestDefault for OneInchApi {
    fn test_default() -> Self {
        Self {
            url: default_one_inch_url(),
            api_key: "test-api-key".to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use {super::*, crate::test_util::TestDefault};

    #[test]
    fn deserialize_defaults() {
        let toml = "";
        let config: PriceEstimation = toml::from_str(toml).unwrap();
        assert!(config.tenderly.is_none());
        assert!(config.price_estimation_rate_limiter.is_none());
        assert!(config.amount_to_estimate_prices_with.is_none());
        assert!(config.one_inch.is_none());
        assert!(config.coin_gecko.is_none());
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
        assert_eq!(config.min_gas_amount_for_unverified_quotes, 0);
        assert_eq!(config.max_gas_amount_for_unverified_quotes, 0);
    }

    #[test]
    fn deserialize_full() {
        let toml = r#"
        quote-inaccuracy-limit = "0.01"
        quote-verification = "enforce-when-possible"
        quote-timeout = "10s"
        tokens-without-verification = ["0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2"]
        amount-to-estimate-prices-with = "1000000000000000000"
        min-gas-amount-for-unverified-quotes = 400000
        max-gas-amount-for-unverified-quotes = 800000

        [price-estimation-rate-limiter]
        back-off-growth-factor = 2.0
        min-back-off = "1s"
        max-back-off = "30s"

        [tenderly]
        user = "my-user"
        project = "my-project"
        api-key = "my-tenderly-key"

        [one-inch]
        api-key = "my-1inch-key"
        url = "https://custom.1inch.dev/"

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

        let tenderly = config.tenderly.as_ref().unwrap();
        assert_eq!(tenderly.user, "my-user");
        assert_eq!(tenderly.project, "my-project");
        assert_eq!(tenderly.api_key, "my-tenderly-key");
        assert!(config.price_estimation_rate_limiter.is_some());
        assert_eq!(
            config.amount_to_estimate_prices_with,
            Some(alloy::primitives::U256::from(1_000_000_000_000_000_000u64))
        );
        let one_inch = config.one_inch.as_ref().unwrap();
        assert_eq!(one_inch.api_key, "my-1inch-key");
        assert_eq!(one_inch.url.as_str(), "https://custom.1inch.dev/");
        let coin_gecko = config.coin_gecko.as_ref().unwrap();
        assert_eq!(coin_gecko.api_key, "my-cg-key");
        assert_eq!(
            coin_gecko.url.as_str(),
            "https://pro-api.coingecko.com/api/v3/simple/token_price"
        );
        let buffered = coin_gecko.buffered.as_ref().unwrap();
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
        assert_eq!(config.min_gas_amount_for_unverified_quotes, 400_000);
        assert_eq!(config.max_gas_amount_for_unverified_quotes, 800_000);
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
            one_inch: Some(OneInchApi {
                api_key: "secret".to_string(),
                ..TestDefault::test_default()
            }),
            coin_gecko: Some(CoinGeckoConfig {
                api_key: "secret".to_string(),
                ..TestDefault::test_default()
            }),
            tenderly: Some(crate::simulator::TenderlyConfig {
                api_key: "secret".to_string(),
                ..TestDefault::test_default()
            }),
            ..Default::default()
        };
        let debug = format!("{config:?}");
        assert!(!debug.contains("secret"));
        assert!(debug.contains("<REDACTED>"));
    }
}
