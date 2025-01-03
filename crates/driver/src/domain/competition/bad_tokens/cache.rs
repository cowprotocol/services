use {
    crate::domain::{competition::bad_tokens::Quality, eth},
    dashmap::DashMap,
    std::{
        sync::Arc,
        time::{Duration, Instant},
    },
};

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
    last_updated: Instant,
    /// whether the token is supported or not
    is_supported: bool,
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
    pub fn update_quality(&self, token: eth::TokenAddress, is_supported: bool, now: Instant) {
        self.0
            .cache
            .entry(token)
            .and_modify(|token| {
                if !is_supported || now.duration_since(token.last_updated) > self.0.max_age {
                    // Only update the value if the cached value is outdated by now or
                    // if the new value is "Unsupported". This means on conflicting updates
                    // we err on the conservative side and assume a token is unsupported.
                    token.is_supported = is_supported;
                }
                token.last_updated = now;
            })
            .or_insert_with(|| CacheEntry {
                is_supported,
                last_updated: now,
            });
    }

    pub fn evict_outdated_entries(&self) {
        let now = Instant::now();
        self.0
            .cache
            .retain(|_, value| now.duration_since(value.last_updated) < self.0.max_age);
    }

    /// Returns the quality of the token if the cached value has not expired
    /// yet.
    pub fn get_quality(&self, token: &eth::TokenAddress, now: Instant) -> Quality {
        let Some(token) = self.0.cache.get(token) else {
            return Quality::Unknown;
        };
        let still_valid = now.duration_since(token.last_updated) > self.0.max_age;
        match (still_valid, token.is_supported) {
            (false, _) => Quality::Unknown,
            (true, true) => Quality::Supported,
            (true, false) => Quality::Unsupported,
        }
    }
}
