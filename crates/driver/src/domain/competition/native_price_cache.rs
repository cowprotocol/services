//! Driver-local cache of native token prices observed in regular auctions,
//! reused to bound absolute slippage when re-encoding fast-path solutions.

use {
    super::auction::{Price, Prices},
    eth_domain_types as eth,
    moka::future::Cache,
    std::{sync::Arc, time::Duration},
};

/// How long an observed native price stays usable for slippage bounding.
const NATIVE_PRICE_TTL: Duration = Duration::from_secs(300);

/// Max number of distinct tokens whose native price is kept.
const MAX_CACHED_NATIVE_PRICES: u64 = 1000;

/// A bounded, chain-global cache of recently observed native prices. Native
/// prices are not settle-account specific, so a single shared instance serves
/// all solvers. Cloning shares the same underlying cache.
#[derive(Debug, Clone)]
pub struct NativePriceCache(Arc<Cache<eth::TokenAddress, Price>>);

impl NativePriceCache {
    pub fn new() -> Self {
        Self(Arc::new(
            Cache::builder()
                .max_capacity(MAX_CACHED_NATIVE_PRICES)
                .time_to_live(NATIVE_PRICE_TTL)
                .build(),
        ))
    }

    /// Record every observed `(token, price)`.
    pub async fn insert_many(&self, prices: impl IntoIterator<Item = (eth::TokenAddress, Price)>) {
        for (token, price) in prices {
            self.0.insert(token, price).await;
        }
    }

    /// The cached price for each of `tokens`, skipping the ones not cached.
    pub async fn get_many(&self, tokens: impl IntoIterator<Item = eth::TokenAddress>) -> Prices {
        let mut prices = Prices::new();
        for token in tokens {
            if let Some(price) = self.0.get(&token).await {
                prices.insert(token, price);
            }
        }
        prices
    }
}

impl Default for NativePriceCache {
    fn default() -> Self {
        Self::new()
    }
}
