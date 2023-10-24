use {
    crate::{domain::competition::auction, infra::notify},
    primitive_types::{H160, U256},
    serde::Serialize,
    serde_with::serde_as,
    std::collections::BTreeSet,
};

impl Notification {
    pub fn new(auction_id: Option<auction::Id>, kind: notify::Kind) -> Self {
        Self {
            auction_id: auction_id.as_ref().map(ToString::to_string),
            kind: match kind {
                notify::Kind::EmptySolution => Kind::EmptySolution,
                notify::Kind::ScoringFailed(notify::ScoreKind::ObjectiveValueNonPositive) => {
                    Kind::ScoringFailed(ScoreKind::ObjectiveValueNonPositive)
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
                }
            },
        }
    }
}

#[serde_as]
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Notification {
    auction_id: Option<String>,
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
}

#[serde_as]
#[derive(Debug, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ScoreKind {
    SuccessProbabilityOutOfRange(f64),
    ObjectiveValueNonPositive,
    ScoreHigherThanObjective { score: U256, objective_value: U256 },
}
