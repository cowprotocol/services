use {super::Solver, crate::domain::competition};

mod notification;

pub use notification::{Kind, Notification};

pub fn empty_solution(solver: &Solver, auction_id: Option<competition::auction::Id>) {
    // prepare data for notification

    solver.notify(auction_id, notification::Kind::EmptySolution);
}

pub fn price_violation(solver: &Solver, auction_id: Option<competition::auction::Id>) {
    // prepare data for notification

    solver.notify(auction_id, notification::Kind::PriceViolation);
}

pub fn scoring_failed(solver: &Solver, auction_id: Option<competition::auction::Id>) {
    // prepare data for notification

    solver.notify(auction_id, notification::Kind::ScoringFailed);
}
