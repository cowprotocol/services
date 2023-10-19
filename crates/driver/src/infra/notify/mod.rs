use {
    super::Solver,
    crate::domain::competition::{auction, solution},
};

mod notification;

pub use notification::{Kind, Notification, SettleKind};

use crate::domain::{eth, mempools::Error};

pub fn empty_solution(solver: &Solver, auction_id: Option<auction::Id>, solution: solution::Id) {
    solver.notify(auction_id, notification::Kind::EmptySolution(solution));
}

pub fn scoring_failed(solver: &Solver, auction_id: Option<auction::Id>) {
    solver.notify(auction_id, notification::Kind::ScoringFailed);
}

pub fn encoding_failed(solver: &Solver, auction_id: Option<auction::Id>, err: &solution::Error) {
    match err {
        solution::Error::UntrustedInternalization(tokens) => {
            solver.notify(
                auction_id,
                notification::Kind::NonBufferableTokensUsed(tokens.clone()),
            );
        }
        solution::Error::SolverAccountInsufficientBalance => {
            solver.notify(
                auction_id,
                notification::Kind::SolverAccountInsufficientBalance,
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

pub fn mempools_executed(
    solver: &Solver,
    auction_id: Option<auction::Id>,
    res: &Result<eth::TxId, Error>,
) {
    let kind = match res {
        Ok(hash) => notification::SettleKind::Settled(hash.clone()),
        Err(Error::Revert(hash)) => notification::SettleKind::Reverted(hash.clone()),
        Err(Error::Other(_)) => notification::SettleKind::Failed,
    };

    solver.notify(auction_id, notification::Kind::Settled(kind));
}
