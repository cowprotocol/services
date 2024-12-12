use {
    super::Order,
    crate::{
        domain::{competition::Auction, eth},
        infra::{self, config::file::BadTokenDetectionCache},
    },
    dashmap::{DashMap, Entry},
    futures::FutureExt,
    model::interaction::InteractionData,
    shared::bad_token::{trace_call::TraceCallDetectorRaw, TokenQuality},
    std::{
        collections::HashMap,
        fmt,
        sync::Arc,
        time::{Duration, Instant},
    },
};

// TODO better comments
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum Quality {
    /// Solver is likely to produce working solutions when computing
    /// routes for this token.
    Supported,
    /// Solver will likely produce failing solutions when computing
    /// routes for this token. This can have many reasons:
    /// * fees on transfer
    /// * token enforces max transfer amount
    /// * trader is deny listed
    /// * bugs in the solidity compiler make it incompatible with the settlement
    ///   contract - see <https://github.com/cowprotocol/services/pull/781>
    /// * probably tons of other reasons
    Unsupported,
}

// TODO: better name (it only looks up stuff)
#[derive(Default)]
pub struct Detector {
    /// manually configured list of supported and unsupported tokens. Only
    /// tokens that get detected incorrectly by the automatic detectors get
    /// listed here and therefore have a higher precedence.
    hardcoded: HashMap<eth::TokenAddress, Quality>,
    /// cache which is shared and updated by multiple bad token detection
    /// mechanisms
    cache: Arc<Cache>,
    simulation_detector: Option<TraceCallDetectorRaw>,
    metrics: Option<Metrics>,
}

impl Detector {
    pub fn with_config(mut self, config: HashMap<eth::TokenAddress, Quality>) -> Self {
        self.hardcoded = config;
        self
    }

    pub fn with_simulation_detector(mut self, eth: &infra::Ethereum) -> Self {
        let detector =
            TraceCallDetectorRaw::new(eth.web3().clone(), eth.contracts().settlement().address());
        self.simulation_detector = Some(detector);
        self
    }

    pub fn with_heuristic_detector(mut self) -> Self {
        self.metrics = Some(Default::default());
        self
    }

    pub fn with_cache(mut self, cache: Arc<Cache>) -> Self {
        self.cache = cache;
        self
    }

    /// Filter all unsupported orders within an Auction
    pub async fn filter_unsupported_orders_in_auction(
        self: Arc<Self>,
        mut auction: Auction,
    ) -> Auction {
        let now = Instant::now();

        let self_clone = self.clone();

        auction
            .filter_orders(move |order| {
                {
                    let self_clone = self_clone.clone();
                    async move {
                        // We first check the token quality:
                        // - If both tokens are supported, the order does is not filtered
                        // - If any of the order tokens is unsupported, the order is filtered
                        // - If the token quality cannot be determined: call
                        //   `determine_sell_token_quality()` to execute the simulation
                        // All of these operations are done within the same `.map()` in order to
                        // avoid iterating twice over the orders vector
                        let tokens_quality = [order.sell.token, order.buy.token]
                            .iter()
                            .map(|token| self_clone.get_token_quality(*token, now))
                            .collect::<Vec<_>>();
                        let both_tokens_supported = tokens_quality
                            .iter()
                            .all(|token_quality| *token_quality == Some(Quality::Supported));
                        let any_token_unsupported = tokens_quality
                            .iter()
                            .any(|token_quality| *token_quality == Some(Quality::Unsupported));

                        // @TODO: remove the bad tokens from the tokens field?

                        // If both tokens are supported, the order does is not filtered
                        if both_tokens_supported {
                            return Some(order);
                        }

                        // If any of the order tokens is unsupported, the order is filtered
                        if any_token_unsupported {
                            return None;
                        }

                        // If the token quality cannot be determined: call
                        // `determine_sell_token_quality()` to execute the simulation
                        if self_clone.determine_sell_token_quality(&order, now).await
                            == Some(Quality::Supported)
                        {
                            return Some(order);
                        }

                        None
                    }
                }
                .boxed()
            })
            .await;

        self.cache.evict_outdated_entries();

        auction
    }

    fn get_token_quality(&self, token: eth::TokenAddress, now: Instant) -> Option<Quality> {
        if let Some(quality) = self.hardcoded.get(&token) {
            return Some(*quality);
        }

        if let Some(quality) = self.cache.get_quality(token, now) {
            return Some(quality);
        }

        if let Some(metrics) = &self.metrics {
            return metrics.get_quality(token);
        }

        None
    }

    async fn determine_sell_token_quality(&self, order: &Order, now: Instant) -> Option<Quality> {
        let Some(detector) = self.simulation_detector.as_ref() else {
            return None;
        };

        if let Some(quality) = self.cache.get_quality(order.sell.token, now) {
            return Some(quality);
        }

        let token = order.sell.token;
        let pre_interactions: Vec<_> = order
            .pre_interactions
            .iter()
            .map(|i| InteractionData {
                target: i.target.0,
                value: i.value.0,
                call_data: i.call_data.0.clone(),
            })
            .collect();

        match detector
            .test_transfer(
                eth::Address::from(order.trader()).0,
                token.0 .0,
                order.sell.amount.0,
                &pre_interactions,
            )
            .await
        {
            Err(err) => {
                tracing::debug!(?err, "failed to determine token quality");
                None
            }
            Ok(TokenQuality::Good) => {
                self.cache.update_quality(token, Quality::Supported, now);
                Some(Quality::Supported)
            }
            Ok(TokenQuality::Bad { reason }) => {
                tracing::debug!(reason, "cache token as unsupported");
                self.cache.update_quality(token, Quality::Unsupported, now);
                Some(Quality::Unsupported)
            }
        }
    }
}

impl fmt::Debug for Detector {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Detector")
            .field("hardcoded", &self.hardcoded)
            .field("dynamic", &format_args!("Vec<Cache>"))
            .finish()
    }
}

/// Cache keeping track of whether or not a token is considered supported or
/// not. Internally reference counted for cheap clones and easy sharing.
/// Stores a map instead of a set to not recompute the quality of good tokens
/// over and over.
/// Evicts cached value after a configurable period of time.
pub struct Cache {
    cache: DashMap<eth::TokenAddress, CacheEntry>,
    /// entries older than this get ignored and evicted
    max_age: Duration,
}

struct CacheEntry {
    /// when the decision on the token quality was made
    timestamp: Instant,
    /// whether the token is supported or not
    quality: Quality,
}

impl Default for Cache {
    fn default() -> Self {
        Self::new(&BadTokenDetectionCache::default())
    }
}

impl Cache {
    /// Creates a new instance which evicts cached values after a period of
    /// time.
    pub fn new(bad_token_detection_cache: &BadTokenDetectionCache) -> Self {
        Self {
            max_age: bad_token_detection_cache.max_age,
            cache: DashMap::with_capacity(bad_token_detection_cache.max_size),
        }
    }

    /// Updates whether or not a token should be considered supported.
    pub fn update_quality(&self, token: eth::TokenAddress, quality: Quality, now: Instant) {
        match self.cache.entry(token) {
            Entry::Occupied(mut o) => {
                let value = o.get_mut();
                if now.duration_since(value.timestamp) > self.max_age
                    || quality == Quality::Unsupported
                {
                    // Only update the value if the cached value is outdated by now or
                    // if the new value is "Unsupported". This means on conflicting updates
                    // we err on the conservative side and assume a token is unsupported.
                    value.quality = quality;
                }
                value.timestamp = now;
            }
            Entry::Vacant(v) => {
                v.insert(CacheEntry {
                    quality,
                    timestamp: now,
                });
            }
        }
    }

    fn evict_outdated_entries(&self) {
        let now = Instant::now();
        self.cache
            .retain(|_, value| now.duration_since(value.timestamp) > self.max_age);
    }

    /// Returns the quality of the token. If the cached value is older than the
    /// `max_age` it gets ignored and the token evicted.
    pub fn get_quality(&self, token: eth::TokenAddress, now: Instant) -> Option<Quality> {
        let Entry::Occupied(entry) = self.cache.entry(token) else {
            return None;
        };

        let value = entry.get();
        if now.duration_since(value.timestamp) > self.max_age {
            entry.remove();
            return None;
        }

        Some(value.quality)
    }
}

#[derive(Default)]
struct Metrics {}

impl Metrics {
    fn get_quality(&self, _token: eth::TokenAddress) -> Option<Quality> {
        todo!()
    }
}
