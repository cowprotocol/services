use {
    super::Quality,
    crate::domain::eth,
    dashmap::DashMap,
    std::{collections::HashSet, sync::Arc},
};

#[derive(Default)]
pub struct Detector(Arc<Inner>);

#[derive(Default)]
struct Inner {
    counter: DashMap<eth::TokenAddress, u64>,
}

impl Detector {
    const UNSUPPORTED_THRESHOLD: u64 = 10;

    pub fn get_quality(&self, token: eth::TokenAddress) -> Option<Quality> {
        self.0
            .counter
            .get(&token)
            .filter(|counter| **counter >= Self::UNSUPPORTED_THRESHOLD)
            .map(|_| Quality::Unsupported)
    }

    pub fn update_failing_tokens(&self, tokens: HashSet<eth::TokenAddress>) {
        for token in tokens {
            self.0
                .counter
                .entry(token)
                .and_modify(|c| *c += 1)
                .or_insert(1);
        }
    }

    pub fn update_successful_tokens(&self, tokens: HashSet<eth::TokenAddress>) {
        for token in tokens {
            self.0.counter.remove(&token);
        }
    }
}
