//! What a host reports about the engine: the rules an evaluation skipped and,
//! while a legacy fee source runs beside the engine, whether the two agree.

use {
    crate::Evaluation,
    std::{
        collections::BTreeMap,
        sync::{Mutex, MutexGuard, Once},
        time::Duration,
    },
};

#[derive(prometheus_metric_storage::MetricStorage)]
#[metric(subsystem = "fee_engine")]
struct Metrics {
    /// Fee decisions by whether the fee engine agreed with the legacy source
    /// (and its fees were served) or not (and the legacy ones were).
    #[metric(labels("check", "result"))]
    comparison: prometheus::IntCounterVec,
}

impl Metrics {
    fn get() -> &'static Self {
        Self::instance(observe::metrics::get_storage_registry())
            .expect("unexpected error getting metrics instance")
    }
}

/// How long events are grouped before they are logged.
const LOG_PERIOD: Duration = Duration::from_secs(1);

static PENDING: Mutex<Pending> = Mutex::new(Pending::new());

/// The events of the current period, by group. A storm of events then costs
/// one log line per group and period, and none is lost.
#[derive(Debug, Default, PartialEq)]
struct Pending {
    /// By check and group: how many, and the first one.
    mismatches: BTreeMap<(&'static str, String), (u64, String)>,
    /// By the skipped rule and its error: how many.
    skipped: BTreeMap<String, u64>,
}

impl Pending {
    const fn new() -> Self {
        Self {
            mismatches: BTreeMap::new(),
            skipped: BTreeMap::new(),
        }
    }

    fn add_mismatch(&mut self, check: &'static str, mismatch: Mismatch) {
        self.mismatches
            .entry((check, mismatch.group))
            .or_insert((0, mismatch.example))
            .0 += 1;
    }

    fn log(self) {
        for ((check, group), (count, example)) in self.mismatches {
            tracing::warn!(
                check,
                count,
                group,
                example,
                "fee engine disagreed with the legacy volume fees; served the legacy ones"
            );
        }
        for (skipped, count) in self.skipped {
            tracing::warn!(count, skipped, "fee engine skipped a rule");
        }
    }
}

/// The pending events, with the thread that logs them once a period started.
fn pending() -> MutexGuard<'static, Pending> {
    static LOGGER: Once = Once::new();
    LOGGER.call_once(|| {
        std::thread::Builder::new()
            .name("fee-engine-log".to_owned())
            .spawn(|| {
                loop {
                    std::thread::sleep(LOG_PERIOD);
                    std::mem::take(&mut *PENDING.lock().unwrap()).log();
                }
            })
            .expect("spawning the fee engine log thread");
    });
    PENDING.lock().unwrap()
}

/// A disagreement as logged. Those with the same `group` are logged together:
/// how many, and the `example` of the first.
pub struct Mismatch {
    pub group: String,
    pub example: String,
}

/// Whether the fee engine's fees are the legacy source's.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    Match,
    Mismatch,
}

impl Outcome {
    fn as_str(self) -> &'static str {
        match self {
            Self::Match => "match",
            Self::Mismatch => "mismatch",
        }
    }

    /// The fees to serve: the engine's on a match, else the legacy ones.
    pub fn fees<T>(self, engine: T, legacy: T) -> T {
        match self {
            Self::Match => engine,
            Self::Mismatch => legacy,
        }
    }
}

/// Publishes the comparison counters of `checks` at zero, so that the first
/// mismatch of a check shows as an increase.
pub fn reset_comparison_counters(checks: &[&str]) {
    for check in checks {
        for outcome in [Outcome::Match, Outcome::Mismatch] {
            Metrics::get()
                .comparison
                .with_label_values(&[check, outcome.as_str()])
                .reset();
        }
    }
}

/// Counts one comparison of `check`. A disagreement is also logged, grouped
/// with the others of its kind once a second.
pub fn report_comparison(
    check: &'static str,
    agree: bool,
    mismatch: impl FnOnce() -> Mismatch,
) -> Outcome {
    let outcome = if agree {
        Outcome::Match
    } else {
        // Running this inside the add_mismatch parens means the pending lock is
        // kept while formatting text
        let mismatch = mismatch();
        pending().add_mismatch(check, mismatch);
        Outcome::Mismatch
    };
    Metrics::get()
        .comparison
        .with_label_values(&[check, outcome.as_str()])
        .inc();
    outcome
}

impl Evaluation<'_> {
    /// Logs the rules the evaluation skipped, grouped with the same skips
    /// once a second.
    pub fn warn_skipped(&self) {
        if self.skipped.is_empty() {
            return;
        }
        let mut pending = pending();
        for skipped in &self.skipped {
            *pending.skipped.entry(skipped.to_string()).or_default() += 1;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn count(check: &str, outcome: Outcome) -> u64 {
        Metrics::get()
            .comparison
            .with_label_values(&[check, outcome.as_str()])
            .get()
    }

    #[test]
    fn counts_every_comparison_and_serves_the_engine_fees_on_a_match() {
        let check = "report_test";
        reset_comparison_counters(&[check]);
        let outcome = report_comparison(check, true, || panic!("a match is not a mismatch"));
        assert_eq!(outcome, Outcome::Match);
        assert_eq!(outcome.fees("engine", "legacy"), "engine");
        for _ in 0..3 {
            let outcome = report_comparison(check, false, || mismatch("group", "example"));
            assert_eq!(outcome, Outcome::Mismatch);
            assert_eq!(outcome.fees("engine", "legacy"), "legacy");
        }
        assert_eq!(count(check, Outcome::Match), 1);
        assert_eq!(count(check, Outcome::Mismatch), 3);
    }

    fn mismatch(group: &str, example: &str) -> Mismatch {
        Mismatch {
            group: group.to_owned(),
            example: example.to_owned(),
        }
    }

    #[test]
    fn groups_mismatches_with_the_first_as_example() {
        let mut pending = Pending::new();
        pending.add_mismatch("quote", mismatch("a", "first"));
        pending.add_mismatch("quote", mismatch("a", "second"));
        pending.add_mismatch("quote", mismatch("b", "third"));
        pending.add_mismatch("fast_path", mismatch("a", "fourth"));
        assert_eq!(
            pending.mismatches,
            BTreeMap::from([
                (("fast_path", "a".to_owned()), (1, "fourth".to_owned())),
                (("quote", "a".to_owned()), (2, "first".to_owned())),
                (("quote", "b".to_owned()), (1, "third".to_owned())),
            ])
        );
    }
}
