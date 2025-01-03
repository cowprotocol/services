use {
    super::Quality,
    crate::domain::eth,
    dashmap::DashMap,
    std::{
        sync::Arc,
        time::{Duration, Instant},
    },
};

#[derive(Default, Debug)]
struct TokenStatistics {
    attempts: u32,
    fails: u32,
    flagged_unsupported_at: Option<Instant>,
}

/// Monitors tokens to determine whether they are considered "unsupported" based
/// on the ratio of failing to total settlement encoding attempts. A token must
/// have participated in at least `REQUIRED_MEASUREMENTS` attempts to be
/// evaluated. If, at that point, the ratio of failures is greater than or equal
/// to `FAILURE_RATIO`, the token is considered unsupported.
#[derive(Clone)]
pub struct Detector {
    failure_ratio: f64,
    required_measurements: u32,
    counter: Arc<DashMap<eth::TokenAddress, TokenStatistics>>,
    log_only: bool,
    token_freeze_time: Duration,
}

impl Detector {
    pub fn new(
        failure_ratio: f64,
        required_measurements: u32,
        log_only: bool,
        token_freeze_time: Duration,
    ) -> Self {
        Self {
            failure_ratio,
            required_measurements,
            counter: Default::default(),
            log_only,
            token_freeze_time,
        }
    }

    pub fn get_quality(&self, token: &eth::TokenAddress, now: Instant) -> Option<Quality> {
        let stats = self.counter.get(token)?;
        if stats
            .flagged_unsupported_at
            .is_some_and(|t| now.duration_since(t) > self.token_freeze_time)
        {
            // Sometimes tokens only cause issues temporarily. If the token's freeze
            // period expired we give it another chance to see if it still behaves badly.
            return None;
        }

        let is_unsupported = self.stats_indicates_unsupported(&stats);
        (!self.log_only && is_unsupported).then_some(Quality::Unsupported)
    }

    fn stats_indicates_unsupported(&self, stats: &TokenStatistics) -> bool {
        let token_failure_ratio = stats.fails as f64 / stats.attempts as f64;
        stats.attempts >= self.required_measurements && token_failure_ratio >= self.failure_ratio
    }

    /// Updates the tokens that participated in settlements by
    /// incrementing their attempt count.
    /// `failure` indicates whether the settlement was successful or not.
    pub fn update_tokens(
        &self,
        token_pairs: &[(eth::TokenAddress, eth::TokenAddress)],
        failure: bool,
    ) {
        let now = Instant::now();
        let mut new_unsupported_tokens = vec![];
        token_pairs
            .iter()
            .flat_map(|(token_a, token_b)| [token_a, token_b])
            .for_each(|token| {
                let mut stats = self
                    .counter
                    .entry(*token)
                    .and_modify(|counter| {
                        counter.attempts += 1;
                        counter.fails += u32::from(failure);
                    })
                    .or_insert_with(|| TokenStatistics {
                        attempts: 1,
                        fails: u32::from(failure),
                        flagged_unsupported_at: None,
                    });

                // token neeeds to be frozen as unsupported for a while
                if self.stats_indicates_unsupported(&stats)
                    && stats
                        .flagged_unsupported_at
                        .is_none_or(|t| now.duration_since(t) > self.token_freeze_time)
                {
                    new_unsupported_tokens.push(token);
                    stats.flagged_unsupported_at = Some(now);
                }
            });

        if !new_unsupported_tokens.is_empty() {
            tracing::debug!(
                tokens = ?new_unsupported_tokens,
                "mark tokens as unsupported"
            );
        }
    }
}
