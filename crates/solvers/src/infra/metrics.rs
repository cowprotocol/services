use {
    crate::domain::{auction, solution},
    chrono::Utc,
};

/// Metrics for the solver engine.
#[derive(Debug, Clone, prometheus_metric_storage::MetricStorage)]
pub struct Metrics {
    /// The amount of time this solver engine has for solving.
    #[metric(buckets(0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15))]
    pub time_limit: prometheus::Histogram,

    /// The amount of time this solver engine has left when it finished solving.
    #[metric(buckets(0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15))]
    pub remaining_time: prometheus::Histogram,

    /// Errors that occurred during solving.
    #[metric(labels("reason"))]
    pub solve_errors: prometheus::IntCounterVec,

    /// The number of solutions that were found.
    pub solutions: prometheus::IntCounter,
}

/// Setup the metrics registry.
pub fn init() {
    observe::metrics::setup_registry_reentrant(Some("solver-engine".to_owned()), None);
}

pub fn solve(auction: &auction::Auction) {
    get().time_limit.observe(remaining_time(&auction.deadline));
}

pub fn solved(deadline: &auction::Deadline, solutions: &[solution::Solution]) {
    get().remaining_time.observe(remaining_time(deadline));
    get().solutions.inc_by(solutions.len() as u64);
}

pub fn solve_error(reason: &str) {
    get().solve_errors.with_label_values(&[reason]).inc();
}

/// Get the metrics instance.
fn get() -> &'static Metrics {
    Metrics::instance(observe::metrics::get_storage_registry())
        .expect("unexpected error getting metrics instance")
}

fn remaining_time(deadline: &auction::Deadline) -> f64 {
    deadline
        .0
        .signed_duration_since(Utc::now())
        .num_milliseconds() as f64
        / 1000.0
}
