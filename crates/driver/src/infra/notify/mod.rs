use {
    super::Solver,
    crate::domain::competition::{auction, solution},
};

mod notification;

pub use notification::{Kind, Notification, ScoreKind};

use crate::{boundary, domain::competition::score};

pub fn empty_solution(solver: &Solver, auction_id: Option<auction::Id>, _solution: solution::Id) {
    solver.notify(auction_id, notification::Kind::EmptySolution);
}

pub fn scoring_failed(solver: &Solver, auction_id: Option<auction::Id>, err: &score::Error) {
    let notification = match err {
        score::Error::ObjectiveValueNonPositive(
            boundary::settlement::Error::ObjectiveValueNonPositive(objective_value),
        ) => notification::Kind::ScoringFailed(notification::ScoreKind::ObjectiveValueNonPositive(
            objective_value.clone(),
        )),
        score::Error::ObjectiveValueNonPositive(_) => return,
        score::Error::ScoreHigherThanObjective(score) => notification::Kind::ScoringFailed(
            notification::ScoreKind::ScoreHigherThanObjective(*score),
        ),
        score::Error::SuccessProbabilityOutOfRange(success_probability) => {
            notification::Kind::ScoringFailed(
                notification::ScoreKind::SuccessProbabilityOutOfRange(*success_probability),
            )
        }
        score::Error::Boundary(_) => return,
    };

    solver.notify(auction_id, notification);
}

pub fn encoding_failed(solver: &Solver, auction_id: Option<auction::Id>, err: &solution::Error) {
    match err {
        solution::Error::UntrustedInternalization(tokens) => {
            solver.notify(
                auction_id,
                notification::Kind::NonBufferableTokensUsed(tokens.clone()),
            );
        }
        solution::Error::SolverAccountInsufficientBalance(required) => {
            solver.notify(
                auction_id,
                notification::Kind::SolverAccountInsufficientBalance(*required),
            );
        }
        solution::Error::Blockchain(_) => (),
        solution::Error::Boundary(_) => (),
        solution::Error::Simulation(_) => (), // todo,
        solution::Error::AssetFlow(_) => (),
        solution::Error::Execution(_) => (),
        solution::Error::FailingInternalization => (),
        solution::Error::DifferentSolvers => (),
    }
}
