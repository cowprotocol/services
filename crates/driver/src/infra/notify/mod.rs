use {
    super::Solver,
    crate::domain::competition::{auction, solution},
};

mod notification;

pub use notification::{Kind, Notification, ScoreKind, Settlement, SimulationSucceededAtLeastOnce};
use {
    super::simulator,
    crate::domain::{competition::score, eth, mempools::Error},
};

pub fn solver_timeout(solver: &Solver, auction_id: Option<auction::Id>) {
    solver.notify(auction_id, None, notification::Kind::Timeout);
}

pub fn empty_solution(solver: &Solver, auction_id: Option<auction::Id>, solution: solution::Id) {
    solver.notify(
        auction_id,
        Some(solution),
        notification::Kind::EmptySolution,
    );
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
        score::Error::ZeroScore => {
            notification::Kind::ScoringFailed(notification::ScoreKind::ZeroScore)
        }
        score::Error::ScoreHigherThanQuality(score, quality) => notification::Kind::ScoringFailed(
            notification::ScoreKind::ScoreHigherThanQuality(*score, *quality),
        ),
        score::Error::RiskAdjusted(score::risk::Error::SuccessProbabilityOutOfRange(
            success_probability,
        )) => notification::Kind::ScoringFailed(
            notification::ScoreKind::SuccessProbabilityOutOfRange(*success_probability),
        ),
        score::Error::RiskAdjusted(score::risk::Error::ObjectiveValueNonPositive(
            quality,
            gas_cost,
        )) => notification::Kind::ScoringFailed(
            notification::ScoreKind::ObjectiveValueNonPositive(*quality, *gas_cost),
        ),
        score::Error::RiskAdjusted(score::risk::Error::Boundary(_)) => return,
        score::Error::Boundary(_) => return,
        score::Error::Scoring(_) => return, // TODO: should we notify?
    };

    solver.notify(auction_id, solution_id, notification);
}

pub fn encoding_failed(
    solver: &Solver,
    auction_id: Option<auction::Id>,
    solution_id: solution::Id,
    err: &solution::Error,
) {
    let notification = match err {
        solution::Error::NonBufferableTokensUsed(tokens) => {
            notification::Kind::NonBufferableTokensUsed(tokens.clone())
        }
        solution::Error::SolverAccountInsufficientBalance(required) => {
            notification::Kind::SolverAccountInsufficientBalance(*required)
        }
        solution::Error::Blockchain(_) => return,
        solution::Error::Boundary(_) => return,
        solution::Error::Simulation(error) => {
            simulation_failed(solver, auction_id, solution_id, error, false);
            return;
        }
        solution::Error::FailingInternalization => return,
        solution::Error::DifferentSolvers => return,
    };

    solver.notify(auction_id, Some(solution_id), notification);
}

pub fn simulation_failed(
    solver: &Solver,
    auction_id: Option<auction::Id>,
    solution_id: solution::Id,
    err: &simulator::Error,
    succeeded_at_least_once: SimulationSucceededAtLeastOnce,
) {
    let kind = match err {
        simulator::Error::Revert(error) => notification::Kind::SimulationFailed(
            error.block,
            error.tx.clone(),
            succeeded_at_least_once,
        ),
        simulator::Error::Other(error) => notification::Kind::DriverError(error.to_string()),
    };
    solver.notify(auction_id, Some(solution_id), kind);
}

pub fn executed(
    solver: &Solver,
    auction_id: auction::Id,
    solution_id: Option<solution::Id>,
    res: &Result<eth::TxId, Error>,
) {
    if solution_id.is_none() {
        return;
    };

    let kind = match res {
        Ok(hash) => notification::Settlement::Success(hash.clone()),
        Err(Error::Revert(hash)) => notification::Settlement::Revert(hash.clone()),
        Err(Error::SimulationRevert) => notification::Settlement::SimulationRevert,
        Err(Error::Other(_) | Error::Expired | Error::Disabled) => notification::Settlement::Fail,
    };

    solver.notify(
        Some(auction_id),
        solution_id,
        notification::Kind::Settled(kind),
    );
}

pub fn duplicated_solution_id(
    solver: &Solver,
    auction_id: Option<auction::Id>,
    solution_id: &solution::Id,
) {
    solver.notify(
        auction_id,
        Some(solution_id.clone()),
        notification::Kind::DuplicatedSolutionId,
    );
}

pub fn postprocessing_timed_out(solver: &Solver, auction_id: Option<auction::Id>) {
    solver.notify(auction_id, None, notification::Kind::PostprocessingTimedOut);
}
