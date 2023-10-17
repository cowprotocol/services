use {
    super::Solver,
    crate::domain::competition::{auction, solution},
};

mod notification;

pub use notification::{Kind, Notification};

pub fn empty_solution(solver: &Solver, auction_id: Option<auction::Id>) {
    solver.notify(auction_id, notification::Kind::EmptySolution);
}

pub fn price_violation(solver: &Solver, auction_id: Option<auction::Id>) {
    solver.notify(auction_id, notification::Kind::PriceViolation);
}

pub fn scoring_failed(solver: &Solver, auction_id: Option<auction::Id>) {
    solver.notify(auction_id, notification::Kind::ScoringFailed);
}

pub fn encoding_failed(
    solver: &Solver,
    auction_id: Option<auction::Id>,
    solution_id: solution::Id,
    err: solution::Error,
) {
    match err {
        solution::Error::UntrustedInternalization => {
            solver.notify(auction_id, notification::Kind::UntrustedInternalization);
        }
        solution::Error::InsufficientBalance => {
            solver.notify(auction_id, notification::Kind::InsufficientBalance);
        }
        _ => (),
    }
}
