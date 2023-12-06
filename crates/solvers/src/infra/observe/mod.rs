use {
    crate::domain::{auction, solution},
    chrono::Utc,
};

pub mod metrics;

pub fn solve(auction: &auction::Auction) {
    metrics::get()
        .time_limit
        .observe(remaining_time(&auction.deadline));
}

pub fn solved(deadline: &auction::Deadline, solutions: &[solution::Solution]) {
    metrics::get()
        .remaining_time
        .observe(remaining_time(deadline));
    metrics::get().solutions.inc_by(solutions.len() as u64);
}

pub fn solve_error(reason: &str) {
    metrics::get()
        .solve_errors
        .with_label_values(&[reason])
        .inc();
}

fn remaining_time(deadline: &auction::Deadline) -> f64 {
    deadline
        .0
        .signed_duration_since(Utc::now())
        .num_milliseconds() as f64
        / 1000.0
}
