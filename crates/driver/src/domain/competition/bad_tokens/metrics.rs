use {super::Quality, crate::domain::eth, dashmap::DashMap, std::sync::Arc};

/// Monitors tokens to determine whether they are considered "unsupported" based
/// on the ratio of failing to total settlement encoding attempts. A token must
/// have participated in at least `REQUIRED_MEASUREMENTS` attempts to be
/// evaluated. If, at that point, the ratio of failures is greater than or equal
/// to `FAILURE_RATIO`, the token is considered unsupported.
#[derive(Default, Clone)]
pub struct Detector(Arc<Inner>);

#[derive(Default)]
struct Inner {
    counter: DashMap<eth::TokenAddress, TokenStatistics>,
}

#[derive(Default)]
struct TokenStatistics {
    attempts: u32,
    fails: u32,
}

impl Detector {
    /// The ratio of failures to attempts that qualifies a token as unsupported.
    const FAILURE_RATIO: f64 = 0.9;
    /// The minimum number of attempts required before evaluating a token’s
    /// quality.
    const REQUIRED_MEASUREMENTS: u32 = 20;

    pub fn get_quality(&self, token: &eth::TokenAddress) -> Option<Quality> {
        let measurements = self.0.counter.get(token)?;
        let is_unsupported = measurements.attempts >= Self::REQUIRED_MEASUREMENTS
            && (measurements.fails as f64 / measurements.attempts as f64) >= Self::FAILURE_RATIO;

        is_unsupported.then_some(Quality::Unsupported)
    }

    /// Updates the tokens that participated in settlements by
    /// incrementing their attempt count.
    /// `failure` indicates whether the settlement was successful or not.
    pub fn update_tokens(
        &self,
        token_pairs: &[(eth::TokenAddress, eth::TokenAddress)],
        failure: bool,
    ) {
        token_pairs
            .iter()
            .flat_map(|(token_a, token_b)| [token_a, token_b])
            .for_each(|token| {
                self.0
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
            });
    }
}
