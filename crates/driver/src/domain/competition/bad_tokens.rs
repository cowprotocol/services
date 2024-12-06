use {
    crate::domain::eth,
    dashmap::{DashMap, Entry},
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
    hardcoded: HashMap<eth::Address, Quality>,
    /// cache which is shared and updated by multiple bad token detection
    /// mechanisms
    dynamic: Vec<Cache>,
}

impl Detector {
    /// Returns which of the passed in tokens should be considered unsupported.
    pub fn supported_tokens(&self, mut tokens: Vec<eth::Address>) -> Vec<eth::Address> {
        let now = Instant::now();

        tokens.retain(|token| {
            if let Some(entry) = self.hardcoded.get(token) {
                return *entry == Quality::Supported;
            }

            for cache in &self.dynamic {
                if let Some(quality) = cache.get_quality(*token, now) {
                    return quality == Quality::Supported;
                }
            }

            // token quality is unknown so we assume it's good
            true
        });

        // now it only contains good tokens
        tokens
    }

    /// Creates a new [`Detector`] with a configured list of token
    /// qualities.
    pub fn new(config: HashMap<eth::Address, Quality>) -> Self {
        Self {
            hardcoded: config,
            dynamic: Default::default(),
        }
    }

    /// Registers an externally managed [`Cache`] to read the quality
    /// of tokens from.
    pub fn register_cache(&mut self, cache: Cache) -> &mut Self {
        self.dynamic.push(cache);
        self
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
struct Cache {
    cache: Arc<DashMap<eth::Address, CacheEntry>>,
    /// entries older than this get ignored and evicted
    max_age: Duration,
    /// evicts entries when the cache grows beyond this size
    max_size: usize,
}

struct CacheEntry {
    /// when the decision on the token quality was made
    timestamp: std::time::Instant,
    /// whether the token is supported or not
    quality: Quality,
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
    pub fn update_tokens(&self, updates: impl IntoIterator<Item = (eth::Address, Quality)>) {
        let now = Instant::now();
        for (token, quality) in updates {
            self.cache.insert(
                token,
                CacheEntry {
                    quality,
                    timestamp: now,
                },
            );
        }

        if self.cache.len() > self.max_size {
            // this could still leave us with more than max_size entries but it at least
            // guarantees that the cache does not grow beyond the actual working set which
            // is enough for now
            self.cache
                .retain(|_, value| now.duration_since(value.timestamp) > self.max_age);
        }
    }

    /// Returns the quality of the token. If the cached value is older than the
    /// `max_age` it gets ignored and the token evicted.
    pub fn get_quality(&self, token: eth::Address, now: Instant) -> Option<Quality> {
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

/// Detects bad a token's quality with simulations using `trace_callMany`.
struct SimulationDetector {
    cache: Cache,
    detector: Arc<TraceCallDetectorRaw>,
}

impl SimulationDetector {
    pub fn new(detector: Arc<TraceCallDetectorRaw>) -> Self {
        Self {
            detector,
            cache: Default::default(),
        }
    }

    pub async fn determine_token_quality(
        &self,
        token: eth::Address,
        holder: eth::Address,
        amount: eth::U256,
        pre_interactions: &[InteractionData],
    ) {
        if self.cache.get_quality(token, Instant::now()).is_some() {
            return;
        }

        match self
            .detector
            .test_transfer(holder.0, token.0, amount, pre_interactions)
            .await
        {
            Err(err) => {
                tracing::debug!(?err, "failed to determine token quality");
            }
            Ok(TokenQuality::Good) => self.cache.update_tokens([(token, Quality::Supported)]),
            Ok(TokenQuality::Bad { reason }) => {
                tracing::debug!(reason, "cache token as unsupported");
                self.cache.update_tokens([(token, Quality::Unsupported)]);
            }
        }
    }
}

/// Keeps track of how often tokens are associated with reverting solutions
/// to detect unsupported tokens based on heuristics. Tokens that are
/// often part of reverting solutions are likely to be unsupported.
struct MetricsDetector {
    cache: Cache,
    metrics: Arc<Metrics>,
}

impl MetricsDetector {
    /// Updates metrics on how often each token is associated with a failing
    /// settlement.
    pub fn record_failed_settlement(&self, tokens: impl IntoIterator<Item = eth::Address>) {}
}

struct Metrics {}
