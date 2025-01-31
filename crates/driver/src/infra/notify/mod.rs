use {
    super::Solver,
    crate::domain::competition::{auction, solution},
};

mod notification;

pub use notification::{
    BanReason,
    Kind,
    Notification,
    ScoreKind,
    Settlement,
    SimulationSucceededAtLeastOnce,
};
use {
    super::simulator,
    crate::domain::{eth, mempools::Error},
};

pub fn solver_timeout(solver: &Solver, auction_id: Option<auction::Id>) {
    solver.notify_and_forget(auction_id, None, notification::Kind::Timeout);
}

pub fn empty_solution(solver: &Solver, auction_id: Option<auction::Id>, solution: solution::Id) {
    solver.notify_and_forget(
        auction_id,
        Some(solution),
        notification::Kind::EmptySolution,
    );
}

pub fn scoring_failed(
    solver: &Solver,
    auction_id: Option<auction::Id>,
    solution_id: &solution::Id,
    err: &solution::error::Scoring,
) {
    let notification = match err {
        solution::error::Scoring::InvalidClearingPrices => {
            notification::Kind::ScoringFailed(ScoreKind::InvalidClearingPrices)
        }
        solution::error::Scoring::Math(_)
        | solution::error::Scoring::CalculateCustomPrices(
            solution::error::Trade::Math(_) | solution::error::Trade::ProtocolFeeOnStaticOrder,
        ) => return,
        solution::error::Scoring::CalculateCustomPrices(
            solution::error::Trade::InvalidExecutedAmount,
        ) => notification::Kind::ScoringFailed(ScoreKind::InvalidExecutedAmount),
        solution::error::Scoring::MissingPrice(token) => {
            notification::Kind::ScoringFailed(ScoreKind::MissingPrice(*token))
        }
    };

    solver.notify_and_forget(auction_id, Some(solution_id.clone()), notification);
}

pub fn encoding_failed(
    solver: &Solver,
    auction_id: Option<auction::Id>,
    solution_id: &solution::Id,
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
        solution::Error::GasLimitExceeded(used, limit) => notification::Kind::DriverError(format!(
            "Settlement gas limit exceeded: used {}, limit {}",
            used.0, limit.0
        )),
        solution::Error::Encoding(_) => return,
    };

    solver.notify_and_forget(auction_id, Some(solution_id.clone()), notification);
}

pub fn simulation_failed(
    solver: &Solver,
    auction_id: Option<auction::Id>,
    solution_id: &solution::Id,
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
    solver.notify_and_forget(auction_id, Some(solution_id.clone()), kind);
}

pub fn executed(
    solver: &Solver,
    auction_id: auction::Id,
    solution_id: &solution::Id,
    res: &Result<eth::TxId, Error>,
) {
    let kind = match res {
        Ok(hash) => notification::Settlement::Success(hash.clone()),
        Err(Error::Revert { tx_id: hash, .. }) => notification::Settlement::Revert(hash.clone()),
        Err(Error::SimulationRevert { .. }) => notification::Settlement::SimulationRevert,
        Err(Error::Expired) => notification::Settlement::Expired,
        Err(Error::Other(_) | Error::Disabled) => notification::Settlement::Fail,
    };

    solver.notify_and_forget(
        Some(auction_id),
        Some(solution_id.clone()),
        notification::Kind::Settled(kind),
    );
}

pub fn duplicated_solution_id(
    solver: &Solver,
    auction_id: Option<auction::Id>,
    solution_id: &solution::Id,
) {
    solver.notify_and_forget(
        auction_id,
        Some(solution_id.clone()),
        notification::Kind::DuplicatedSolutionId,
    );
}

pub fn postprocessing_timed_out(solver: &Solver, auction_id: Option<auction::Id>) {
    solver.notify_and_forget(auction_id, None, notification::Kind::PostprocessingTimedOut);
}
