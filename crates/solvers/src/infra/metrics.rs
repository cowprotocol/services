use crate::domain::{auction, solution};

/// Metrics for the solver engine.
#[derive(Debug, Clone, prometheus_metric_storage::MetricStorage)]
#[metric(subsystem = "solver_engine")]
struct Metrics {
    /// The amount of time this solver engine has for solving.
    #[metric(buckets(0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15))]
    time_limit: prometheus::Histogram,

    /// The amount of time this solver engine has left when it finished solving.
    #[metric(buckets(0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15))]
    remaining_time: prometheus::Histogram,

    /// Errors that occurred during solving.
    #[metric(labels("reason"))]
    solve_errors: prometheus::IntCounterVec,

    /// The number of solutions that were found.
    solutions: prometheus::IntCounter,
}

/// Setup the metrics registry.
pub fn init() {
    observe::metrics::setup_registry_reentrant(Some("solver-engine".to_owned()), None);
}

pub fn solve(auction: &auction::Auction) {
    get().time_limit.observe(
        auction
            .deadline
            .remaining()
            .unwrap_or_default()
            .as_secs_f64(),
    );
}

pub fn solved(deadline: &auction::Deadline, solutions: &[solution::Solution]) {
    get()
        .remaining_time
        .observe(deadline.remaining().unwrap_or_default().as_secs_f64());
    get().solutions.inc_by(solutions.len() as u64);
}

/// Get the metrics instance.
fn get() -> &'static Metrics {
    Metrics::instance(observe::metrics::get_storage_registry())
        .expect("unexpected error getting metrics instance")
}
