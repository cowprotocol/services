use {
    super::Order,
    crate::{
        domain::{self, eth},
        infra,
    },
    anyhow::Result,
    dashmap::{DashMap, Entry, OccupiedEntry, VacantEntry},
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

    pub fn filter_unsupported_orders(&self, mut orders: Vec<Order>) -> Vec<Order> {
        let now = Instant::now();

        // group by sell tokens?
        // future calling `determine_sell_token_quality()` for all of orders

        orders.retain(|o| {
            [o.sell.token, o.buy.token].iter().all(|token| {
                self.get_token_quality(*token, now)
                    .is_none_or(|q| q == Quality::Supported)
            })
        });

        self.cache.evict_outdated_entries();

        orders
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

    pub async fn determine_sell_token_quality(
        &self,
        detector: &TraceCallDetectorRaw,
        order: &Order,
        now: Instant,
    ) -> Option<Quality> {
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
                order.trader().0 .0,
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
    /// evicts entries when the cache grows beyond this size
    max_size: usize,
}

struct CacheEntry {
    /// when the decision on the token quality was made
    timestamp: Instant,
    /// whether the token is supported or not
    quality: Quality,
}

impl Default for Cache {
    fn default() -> Self {
        Self::new(Duration::from_secs(60 * 10), 1000)
    }
}

impl Cache {
    /// Creates a new instance which evicts cached values after a period of
    /// time.
    pub fn new(max_age: Duration, max_size: usize) -> Self {
        Self {
            max_age,
            max_size,
            cache: Default::default(),
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
    fn get_quality(&self, token: eth::TokenAddress) -> Option<Quality> {
        todo!()
    }
}
