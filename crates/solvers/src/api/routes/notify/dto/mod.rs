pub use solvers_dto::notification::Notification;

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
        solution_id: notification.solution_id.map(Into::into),
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
            solvers_dto::notification::Kind::ObjectiveValueNonPositive { quality, gas_cost } => {
                notification::Kind::ScoringFailed(
                    notification::ScoreKind::ObjectiveValueNonPositive(
                        (*quality).into(),
                        (*gas_cost).into(),
                    ),
                )
            }
            solvers_dto::notification::Kind::ZeroScore => {
                notification::Kind::ScoringFailed(notification::ScoreKind::ZeroScore)
            }
            solvers_dto::notification::Kind::ScoreHigherThanQuality { score, quality } => {
                notification::Kind::ScoringFailed(notification::ScoreKind::ScoreHigherThanQuality(
                    (*score).into(),
                    (*quality).into(),
                ))
            }
            solvers_dto::notification::Kind::SuccessProbabilityOutOfRange { probability } => {
                notification::Kind::ScoringFailed(
                    notification::ScoreKind::SuccessProbabilityOutOfRange((*probability).into()),
                )
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
