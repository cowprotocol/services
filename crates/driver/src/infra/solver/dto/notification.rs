use {
    crate::{
        domain::{competition::auction, eth},
        domain::competition::solution,
        infra::notify,
    },
    primitive_types::{H160, U256},
    serde::Serialize,
    serde_with::serde_as,
    std::collections::BTreeSet,
};

impl Notification {
    pub fn new(
        auction_id: Option<auction::Id>,
        solution_id: solution::Id,
        kind: notify::Kind,
    ) -> Self {
        Self {
            auction_id: auction_id.as_ref().map(ToString::to_string),
            solution_id: solution_id.0,
            kind: match kind {
                notify::Kind::EmptySolution => Kind::EmptySolution,
                notify::Kind::ScoringFailed(notify::ScoreKind::ObjectiveValueNonPositive) => {
                    Kind::ScoringFailed(ScoreKind::ObjectiveValueNonPositive)
                }
                notify::Kind::ScoringFailed(notify::ScoreKind::ZeroScore) => {
                    Kind::ScoringFailed(ScoreKind::ZeroScore)
                }
                notify::Kind::ScoringFailed(notify::ScoreKind::ScoreHigherThanObjective(
                    score,
                    objective_value,
                )) => Kind::ScoringFailed(ScoreKind::ScoreHigherThanObjective {
                    score: score.0,
                    objective_value: objective_value.0,
                }),
                notify::Kind::ScoringFailed(notify::ScoreKind::SuccessProbabilityOutOfRange(
                    success_probability,
                )) => Kind::ScoringFailed(ScoreKind::SuccessProbabilityOutOfRange(
                    success_probability.0,
                )),
                notify::Kind::NonBufferableTokensUsed(tokens) => Kind::NonBufferableTokensUsed(
                    tokens.into_iter().map(|token| token.0 .0).collect(),
                ),
                notify::Kind::SolverAccountInsufficientBalance(required) => {
                    Kind::SolverAccountInsufficientBalance(required.0)
                },
                notify::Kind::DuplicatedSolutionId => Kind::DuplicatedSolutionId,
                notify::Kind::Settled(kind) => Kind::Settled(match kind {
                    notify::Settlement::Success(hash) => Settlement::Success(hash.0),
                    notify::Settlement::Revert(hash) => Settlement::Revert(hash.0),
                    notify::Settlement::Fail => Settlement::Fail,
                }),
            },
        }
    }
}

#[serde_as]
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Notification {
    auction_id: Option<String>,
    solution_id: u64,
    kind: Kind,
}

#[serde_as]
#[derive(Debug, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    EmptySolution,
    ScoringFailed(ScoreKind),
    NonBufferableTokensUsed(BTreeSet<H160>),
    SolverAccountInsufficientBalance(U256),
    DuplicatedSolutionId,
    Settled(Settlement),
}

#[serde_as]
#[derive(Debug, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ScoreKind {
    ZeroScore,
    ObjectiveValueNonPositive,
    SuccessProbabilityOutOfRange(f64),
    ScoreHigherThanObjective { score: U256, objective_value: U256 },
}

#[serde_as]
#[derive(Debug, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Settlement {
    Success(eth::H256),
    Revert(eth::H256),
    Fail,
}


