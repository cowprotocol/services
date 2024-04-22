pub use solvers_dto::notification::{Notification, SolutionId};

use crate::domain::{auction, eth, notification};

/// Converts a data transfer object into its domain object representation.
pub fn to_domain(
    notification: &solvers_dto::notification::Notification,
) -> notification::Notification {
    notification::Notification {
        auction_id: match notification.auction_id {
            Some(id) => auction::Id::Solve(id),
            None => auction::Id::Quote,
        },
        solution_id: notification.solution_id.as_ref().map(|id| match id {
            SolutionId::Single(id) => notification::Id::Single((*id).into()),
            SolutionId::Merged(ids) => notification::Id::Merged(ids.to_vec()),
        }),
        kind: match &notification.kind {
            solvers_dto::notification::Kind::Timeout => notification::Kind::Timeout,
            solvers_dto::notification::Kind::EmptySolution => notification::Kind::EmptySolution,
            solvers_dto::notification::Kind::SimulationFailed {
                block,
                tx,
                succeeded_once,
            } => notification::Kind::SimulationFailed(
                *block,
                eth::Tx {
                    from: tx.from.into(),
                    to: tx.to.into(),
                    input: tx.input.clone().into(),
                    value: tx.value.into(),
                    access_list: tx.access_list.clone(),
                },
                *succeeded_once,
            ),
            solvers_dto::notification::Kind::InvalidClearingPrices => {
                notification::Kind::ScoringFailed(notification::ScoreKind::InvalidClearingPrices)
            }
            solvers_dto::notification::Kind::MissingPrice { token_address } => {
                notification::Kind::ScoringFailed(notification::ScoreKind::MissingPrice(
                    (*token_address).into(),
                ))
            }
            solvers_dto::notification::Kind::InvalidExecutedAmount => {
                notification::Kind::ScoringFailed(notification::ScoreKind::InvalidExecutedAmount)
            }
            solvers_dto::notification::Kind::NonBufferableTokensUsed { tokens } => {
                notification::Kind::NonBufferableTokensUsed(
                    tokens
                        .clone()
                        .into_iter()
                        .map(|token| token.into())
                        .collect(),
                )
            }
            solvers_dto::notification::Kind::SolverAccountInsufficientBalance { required } => {
                notification::Kind::SolverAccountInsufficientBalance(eth::Ether(*required))
            }
            solvers_dto::notification::Kind::DuplicatedSolutionId => {
                notification::Kind::DuplicatedSolutionId
            }
            solvers_dto::notification::Kind::Success { transaction } => {
                notification::Kind::Settled(notification::Settlement::Success(*transaction))
            }
            solvers_dto::notification::Kind::Revert { transaction } => {
                notification::Kind::Settled(notification::Settlement::Revert(*transaction))
            }
            solvers_dto::notification::Kind::DriverError { reason } => {
                notification::Kind::DriverError(reason.clone())
            }
            solvers_dto::notification::Kind::Cancelled => {
                notification::Kind::Settled(notification::Settlement::SimulationRevert)
            }
            solvers_dto::notification::Kind::Fail => {
                notification::Kind::Settled(notification::Settlement::Fail)
            }
            solvers_dto::notification::Kind::PostprocessingTimedOut => {
                notification::Kind::PostprocessingTimedOut
            }
        },
    }
}
