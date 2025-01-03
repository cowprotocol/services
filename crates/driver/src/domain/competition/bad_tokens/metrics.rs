use {super::Quality, crate::domain::eth, dashmap::DashMap, std::sync::Arc};

#[derive(Default)]
struct TokenStatistics {
    attempts: u32,
    fails: u32,
}

#[derive(Default, Clone)]
pub struct DetectorBuilder(Arc<DashMap<eth::TokenAddress, TokenStatistics>>);

impl DetectorBuilder {
    pub fn build(self, failure_ratio: f64, required_measurements: u32, log_only: bool) -> Detector {
        Detector {
            failure_ratio,
            required_measurements,
            counter: self.0,
            log_only,
        }
    }
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
}

impl Detector {
    pub fn get_quality(&self, token: &eth::TokenAddress) -> Option<Quality> {
        let measurements = self.counter.get(token)?;
        let is_unsupported = self.is_unsupported(&measurements);
        (!self.log_only && is_unsupported).then_some(Quality::Unsupported)
    }

    fn is_unsupported(&self, measurements: &TokenStatistics) -> bool {
        let token_failure_ratio = measurements.fails as f64 / measurements.attempts as f64;
        measurements.attempts >= self.required_measurements
            && token_failure_ratio >= self.failure_ratio
    }

    /// Updates the tokens that participated in settlements by
    /// incrementing their attempt count.
    /// `failure` indicates whether the settlement was successful or not.
    pub fn update_tokens(
        &self,
        token_pairs: &[(eth::TokenAddress, eth::TokenAddress)],
        failure: bool,
    ) {
        let mut unsupported_tokens = vec![];
        token_pairs
            .iter()
            .flat_map(|(token_a, token_b)| [token_a, token_b])
            .for_each(|token| {
                let measurement = self
                    .counter
                    .entry(*token)
                    .and_modify(|counter| {
                        counter.attempts += 1;
                        counter.fails += u32::from(failure)
                    })
                    .or_insert_with(|| TokenStatistics {
                        attempts: 1,
                        fails: u32::from(failure),
                    });
                if self.is_unsupported(&measurement) {
                    unsupported_tokens.push(token);
                }
            });

        if !unsupported_tokens.is_empty() {
            tracing::debug!(
                tokens = ?unsupported_tokens,
                "mark tokens as unsupported"
            );
        }
    }
}
