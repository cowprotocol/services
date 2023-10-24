use {
    super::Solver,
    crate::domain::competition::{auction, solution},
};

mod notification;

pub use notification::{Kind, Notification, ScoreKind};
use {super::simulator, crate::domain::competition::score};

pub fn empty_solution(solver: &Solver, auction_id: Option<auction::Id>, solution: solution::Id) {
    solver.notify(auction_id, solution, notification::Kind::EmptySolution);
}

pub fn scoring_failed(
    solver: &Solver,
    auction_id: Option<auction::Id>,
    solution_id: Option<solution::Id>,
    err: &score::Error,
) {
    if solution_id.is_none() {
        return;
    }

    let notification = match err {
        score::Error::ObjectiveValueNonPositive => {
            notification::Kind::ScoringFailed(notification::ScoreKind::ObjectiveValueNonPositive)
        }
        score::Error::ZeroScore => {
            notification::Kind::ScoringFailed(notification::ScoreKind::ZeroScore)
        }
        score::Error::ScoreHigherThanObjective(score, objective_value) => {
            notification::Kind::ScoringFailed(notification::ScoreKind::ScoreHigherThanObjective(
                *score,
                *objective_value,
            ))
        }
        score::Error::SuccessProbabilityOutOfRange(success_probability) => {
            notification::Kind::ScoringFailed(
                notification::ScoreKind::SuccessProbabilityOutOfRange(*success_probability),
            )
        }
        score::Error::Boundary(_) => return,
    };

    solver.notify(auction_id, solution_id.unwrap(), notification);
}

pub fn encoding_failed(
    solver: &Solver,
    auction_id: Option<auction::Id>,
    solution_id: solution::Id,
    err: &solution::Error,
) {
    let notification = match err {
        solution::Error::UntrustedInternalization(tokens) => {
            notification::Kind::NonBufferableTokensUsed(tokens.clone())
        }
        solution::Error::SolverAccountInsufficientBalance(required) => {
            notification::Kind::SolverAccountInsufficientBalance(*required)
        }
        solution::Error::Blockchain(_) => return,
        solution::Error::Boundary(_) => return,
        solution::Error::Simulation(simulator::Error::WithTx(error)) => {
            notification::Kind::SimulationFailed(error.tx.clone())
        }
        solution::Error::Simulation(simulator::Error::Basic(_)) => return,
        solution::Error::AssetFlow(_) => return,
        solution::Error::Execution(_) => return,
        solution::Error::FailingInternalization => return,
        solution::Error::DifferentSolvers => return,
    };

    solver.notify(auction_id, solution_id, notification);
}

pub fn duplicated_solution_id(
    solver: &Solver,
    auction_id: Option<auction::Id>,
    solution_id: solution::Id,
) {
    solver.notify(
        auction_id,
        solution_id,
        notification::Kind::DuplicatedSolutionId,
    );
}
