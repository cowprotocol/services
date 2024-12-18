use {
    super::Quality,
    crate::domain::eth,
    dashmap::DashMap,
    std::{collections::HashSet, sync::Arc},
};

/// Monitors tokens to determine whether they are considered "unsupported" based
/// on the number of consecutive participation in failing settlement encoding.
/// Tokens that consistently participate in failures beyond a predefined
/// threshold are marked as unsupported. Once token participates in a successful
/// settlement encoding, it is removed from the cache.
#[derive(Default)]
pub struct Detector(Arc<Inner>);

#[derive(Default)]
struct Inner {
    counter: DashMap<eth::TokenAddress, u8>,
}

impl Detector {
    /// Defines the threshold for the number of consecutive unsupported
    /// quality detections before a token is considered unsupported.
    const UNSUPPORTED_THRESHOLD: u8 = 5;

    pub fn get_quality(&self, token: eth::TokenAddress) -> Option<Quality> {
        self.0
            .counter
            .get(&token)
            .filter(|counter| **counter >= Self::UNSUPPORTED_THRESHOLD)
            .map(|_| Quality::Unsupported)
    }

    /// Increments the counter of failures for each token.
    pub fn update_failing_tokens(&self, tokens: HashSet<eth::TokenAddress>) {
        for token in tokens {
            self.0
                .counter
                .entry(token)
                .and_modify(|counter| *counter = Self::UNSUPPORTED_THRESHOLD.min(*counter + 1))
                .or_insert(1);
        }
    }

    /// Removes tokens from the cache since they all participated in a
    /// successful settlement encoding.
    pub fn update_successful_tokens(&self, tokens: HashSet<eth::TokenAddress>) {
        for token in tokens {
            self.0.counter.remove(&token);
        }
    }
}
