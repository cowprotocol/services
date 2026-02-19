use crate::{
    domain::competition::{auction, solution},
    infra::notify,
};

pub fn new(
    auction_id: Option<auction::Id>,
    solution_id: Option<solution::Id>,
    kind: notify::Kind,
) -> solvers_dto::notification::Notification {
    solvers_dto::notification::Notification {
        auction_id: auction_id.as_ref().map(|id| id.0),
        solution_id: solution_id.map(solution_id_from_domain),
        kind: match kind {
            notify::Kind::Timeout => solvers_dto::notification::Kind::Timeout,
            notify::Kind::EmptySolution => solvers_dto::notification::Kind::EmptySolution,
            notify::Kind::SimulationFailed(block, tx, succeeded_once) => {
                solvers_dto::notification::Kind::SimulationFailed {
                    block: block.0,
                    tx: solvers_dto::notification::Tx {
                        from: tx.from,
                        to: tx.to,
                        input: tx.input.into(),
                        value: tx.value.0,
                        access_list: tx.access_list.into(),
                    },
                    succeeded_once,
                }
            }
            notify::Kind::ScoringFailed(scoring) => scoring.into(),
            notify::Kind::NonBufferableTokensUsed(tokens) => {
                solvers_dto::notification::Kind::NonBufferableTokensUsed {
                    tokens: tokens.into_iter().map(|token| token.0.0).collect(),
                }
            }
            notify::Kind::SolverAccountInsufficientBalance(required) => {
                solvers_dto::notification::Kind::SolverAccountInsufficientBalance {
                    required: required.0,
                }
            }
            notify::Kind::DuplicatedSolutionId => {
                solvers_dto::notification::Kind::DuplicatedSolutionId
            }
            notify::Kind::DriverError(reason) => {
                solvers_dto::notification::Kind::DriverError { reason }
            }
            notify::Kind::Settled(kind) => match kind {
                notify::Settlement::Success(hash) => solvers_dto::notification::Kind::Success {
                    transaction: hash.0,
                },
                notify::Settlement::Revert(hash) => solvers_dto::notification::Kind::Revert {
                    transaction: hash.0,
                },
                notify::Settlement::SimulationRevert => solvers_dto::notification::Kind::Cancelled,
                notify::Settlement::Fail => solvers_dto::notification::Kind::Fail,
                notify::Settlement::Expired => solvers_dto::notification::Kind::Expired,
            },
            notify::Kind::PostprocessingTimedOut => {
                solvers_dto::notification::Kind::PostprocessingTimedOut
            }
            notify::Kind::DeserializationError(reason) => {
                solvers_dto::notification::Kind::DeserializationError { reason }
            }
        },
    }
}

fn solution_id_from_domain(id: solution::Id) -> solvers_dto::notification::SolutionId {
    match id.solutions().len() {
        1 => solvers_dto::notification::SolutionId::Single(*id.solutions().first().unwrap()),
        _ => solvers_dto::notification::SolutionId::Merged(id.solutions().to_vec()),
    }
}

impl From<notify::ScoreKind> for solvers_dto::notification::Kind {
    fn from(value: notify::ScoreKind) -> Self {
        match value {
            notify::ScoreKind::InvalidClearingPrices => {
                solvers_dto::notification::Kind::InvalidClearingPrices
            }
            notify::ScoreKind::InvalidExecutedAmount => {
                solvers_dto::notification::Kind::InvalidExecutedAmount
            }
            notify::ScoreKind::MissingPrice(token_address) => {
                solvers_dto::notification::Kind::MissingPrice {
                    token_address: token_address.0.0,
                }
            }
        }
    }
}
