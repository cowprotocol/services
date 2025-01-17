use {
    super::Quality,
    crate::{
        domain::eth,
        infra::{observe::metrics, solver},
    },
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
    solver: solver::Name,
}

impl Detector {
    pub fn new(
        failure_ratio: f64,
        required_measurements: u32,
        log_only: bool,
        token_freeze_time: Duration,
        solver: solver::Name,
    ) -> Self {
        Self {
            failure_ratio,
            required_measurements,
            counter: Default::default(),
            log_only,
            token_freeze_time,
            solver,
        }
    }

    pub fn get_quality(&self, token: &eth::TokenAddress, now: Instant) -> Quality {
        let Some(stats) = self.counter.get(token) else {
            return Quality::Unknown;
        };

        if stats
            .flagged_unsupported_at
            .is_some_and(|t| now.duration_since(t) > self.token_freeze_time)
        {
            // Sometimes tokens only cause issues temporarily. If the token's freeze
            // period expired we pretend we don't have enough information to give it
            // another chance. If it still behaves badly it will get frozen immediately.
            return Quality::Unknown;
        }

        match self.log_only {
            true => Quality::Supported,
            false => self.quality_based_on_stats(&stats),
        }
    }

    fn quality_based_on_stats(&self, stats: &TokenStatistics) -> Quality {
        if stats.attempts < self.required_measurements {
            return Quality::Unknown;
        }
        let token_failure_ratio = f64::from(stats.fails) / f64::from(stats.attempts);
        match token_failure_ratio >= self.failure_ratio {
            true => Quality::Unsupported,
            false => Quality::Supported,
        }
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
                if self.quality_based_on_stats(&stats) == Quality::Unsupported
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
            metrics::get()
                .bad_tokens_detected
                .with_label_values(&[&self.solver.0, "metrics"])
                .inc_by(new_unsupported_tokens.len() as u64);
        }
    }
}

#[cfg(test)]
mod tests {
    use {super::*, ethcontract::H160};

    /// Tests that a token only gets marked temporarily as unsupported.
    /// After the freeze period it will be allowed again.
    #[tokio::test]
    async fn unfreeze_bad_tokens() {
        const FREEZE_DURATION: Duration = Duration::from_millis(50);
        let detector = Detector::new(
            0.5,
            2,
            false,
            FREEZE_DURATION,
            solver::Name("mysolver".to_string()),
        );

        let token_a = eth::TokenAddress(eth::ContractAddress(H160([1; 20])));
        let token_b = eth::TokenAddress(eth::ContractAddress(H160([2; 20])));
        let token_quality = || detector.get_quality(&token_a, Instant::now());

        // token is reported as unknown while we don't have enough measurements
        assert_eq!(token_quality(), Quality::Unknown);
        detector.update_tokens(&[(token_a, token_b)], true);
        assert_eq!(token_quality(), Quality::Unknown);
        detector.update_tokens(&[(token_a, token_b)], true);

        // after we got enough measurements the token gets marked as bad
        assert_eq!(token_quality(), Quality::Unsupported);

        // after the freeze period is over the token gets reported as unknown again
        tokio::time::sleep(FREEZE_DURATION).await;
        assert_eq!(token_quality(), Quality::Unknown);

        // after an unfreeze another bad measurement is enough to freeze it again
        detector.update_tokens(&[(token_a, token_b)], true);
        assert_eq!(token_quality(), Quality::Unsupported);
    }
}
