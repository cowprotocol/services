use {
    super::Order,
    crate::{
        domain::{competition::Auction, eth},
        infra::{self, config::file::BadTokenDetection},
    },
    dashmap::{DashMap, Entry},
    futures::StreamExt,
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
    cache: Cache,
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

    pub fn with_cache(mut self, cache: Cache) -> Self {
        self.cache = cache;
        self
    }

    /// Removes all unsupported orders from the auction.
    pub async fn filter_unsupported_orders_in_auction(&self, mut auction: Auction) -> Auction {
        let now = Instant::now();

        let filtered_orders = futures::stream::iter(auction.orders.into_iter())
            .filter_map(move |order| async move {
                let sell = self.get_token_quality(order.sell.token, now);
                let buy = self.get_token_quality(order.sell.token, now);
                match (sell, buy) {
                    // both tokens supported => keep order
                    (Some(Quality::Supported), Some(Quality::Supported)) => Some(order),
                    // at least 1 token unsupported => drop order
                    (Some(Quality::Unsupported), _) | (_, Some(Quality::Unsupported)) => None,
                    // sell token quality is unknown => keep order if token is supported
                    (None, _) => {
                        let quality = self.determine_sell_token_quality(&order, now).await;
                        (quality == Some(Quality::Supported)).then_some(order)
                    }
                    // buy token quality is unknown => keep order (because we can't
                    // determine quality and assume it's good)
                    (_, None) => Some(order),
                }
            })
            .collect::<Vec<_>>()
            .await;
        auction.orders = filtered_orders;

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
        let detector = self.simulation_detector.as_ref()?;

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
#[derive(Clone)]
pub struct Cache(Arc<Inner>);

struct Inner {
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
        Self::new(BadTokenDetection::default().max_age)
    }
}

impl Cache {
    /// Creates a new instance which evicts cached values after a period of
    /// time.
    pub fn new(max_age: Duration) -> Self {
        Self(Arc::new(Inner {
            max_age,
            cache: DashMap::default(),
        }))
    }

    /// Updates whether or not a token should be considered supported.
    fn update_quality(&self, token: eth::TokenAddress, quality: Quality, now: Instant) {
        match self.0.cache.entry(token) {
            Entry::Occupied(mut o) => {
                let value = o.get_mut();
                if now.duration_since(value.timestamp) > self.0.max_age
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
        self.0
            .cache
            .retain(|_, value| now.duration_since(value.timestamp) > self.0.max_age);
    }

    /// Returns the quality of the token. If the cached value is older than the
    /// `max_age` it gets ignored and the token evicted.
    fn get_quality(&self, token: eth::TokenAddress, now: Instant) -> Option<Quality> {
        let Entry::Occupied(entry) = self.0.cache.entry(token) else {
            return None;
        };

        let value = entry.get();
        if now.duration_since(value.timestamp) > self.0.max_age {
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
