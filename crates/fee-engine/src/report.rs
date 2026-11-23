//! What a host reports about the engine: the rules an evaluation skipped and
//! the time an evaluation took.

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
    /// Time the fee engine took to decide the fees of one order.
    #[metric(buckets(
        0.0000005, 0.000001, 0.000002, 0.000005, 0.00001, 0.00002, 0.00005, 0.0001, 0.001
    ))]
    evaluation_time: prometheus::Histogram,

    /// Rules that failed on an order and were skipped, by rule name.
    #[metric(labels("rule"))]
    skipped_rules: prometheus::IntCounterVec,
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
    /// By the skipped rule and its error: how many.
    skipped: BTreeMap<String, u64>,
}

impl Pending {
    const fn new() -> Self {
        Self {
            skipped: BTreeMap::new(),
        }
    }

    fn log(self) {
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
                    // A temporary guard would stay locked until the `;`.
                    let pending = std::mem::take(&mut *PENDING.lock().unwrap());
                    pending.log();
                }
            })
            .expect("spawning the fee engine log thread");
    });
    PENDING.lock().unwrap()
}

/// Runs `evaluate`, the fee decision on one order, and records the time it
/// took.
pub fn time_evaluation<T>(evaluate: impl FnOnce() -> T) -> T {
    let _timer = Metrics::get().evaluation_time.start_timer();
    evaluate()
}

impl Evaluation<'_> {
    /// Counts the rules the evaluation skipped and logs them, grouped with
    /// the same skips once a second.
    pub fn warn_skipped(&self) {
        if self.skipped.is_empty() {
            return;
        }
        let mut pending = pending();
        for skipped in &self.skipped {
            Metrics::get()
                .skipped_rules
                .with_label_values(&[skipped.rule.name()])
                .inc();
            *pending.skipped.entry(skipped.to_string()).or_default() += 1;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn times_every_evaluation() {
        let samples = || Metrics::get().evaluation_time.get_sample_count();
        let before = samples();
        assert_eq!(time_evaluation(|| "fees"), "fees");
        time_evaluation(|| ());
        assert_eq!(samples(), before + 2);
    }
}
