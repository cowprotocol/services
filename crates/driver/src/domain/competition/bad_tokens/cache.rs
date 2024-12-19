use {
    crate::domain::{competition::bad_tokens::Quality, eth},
    dashmap::{DashMap, Entry},
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
    timestamp: Instant,
    /// whether the token is supported or not
    quality: Quality,
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
    pub fn update_quality(&self, token: eth::TokenAddress, quality: Quality, now: Instant) {
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

    pub fn evict_outdated_entries(&self) {
        let now = Instant::now();
        self.0
            .cache
            .retain(|_, value| now.duration_since(value.timestamp) < self.0.max_age);
    }

    /// Returns the quality of the token. If the cached value is older than the
    /// `max_age` it gets ignored and the token evicted.
    pub fn get_quality(&self, token: eth::TokenAddress, now: Instant) -> Option<Quality> {
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
