use {
    crate::domain::{auction, eth, notification},
    ethereum_types::{H160, U256},
    serde::Deserialize,
    serde_with::{serde_as, DisplayFromStr},
    std::collections::BTreeSet,
};

impl Notification {
    /// Converts a data transfer object into its domain object representation.
    pub fn to_domain(&self) -> notification::Notification {
        notification::Notification {
            auction_id: match self.auction_id {
                Some(id) => auction::Id::Solve(id),
                None => auction::Id::Quote,
            },
            solution_id: self.solution_id.into(),
            kind: match &self.kind {
                Kind::EmptySolution => notification::Kind::EmptySolution,
                Kind::ScoringFailed(ScoreKind::ObjectiveValueNonPositive) => {
                    notification::Kind::ScoringFailed(
                        notification::ScoreKind::ObjectiveValueNonPositive,
                    )
                }
                Kind::ScoringFailed(ScoreKind::ZeroScore) => {
                    notification::Kind::ScoringFailed(notification::ScoreKind::ZeroScore)
                }
                Kind::ScoringFailed(ScoreKind::ScoreHigherThanObjective {
                    score,
                    objective_value,
                }) => notification::Kind::ScoringFailed(
                    notification::ScoreKind::ScoreHigherThanObjective(
                        (*score).into(),
                        (*objective_value).into(),
                    ),
                ),
                Kind::ScoringFailed(ScoreKind::SuccessProbabilityOutOfRange(probability)) => {
                    notification::Kind::ScoringFailed(
                        notification::ScoreKind::SuccessProbabilityOutOfRange(
                            (*probability).into(),
                        ),
                    )
                }
                Kind::NonBufferableTokensUsed(tokens) => {
                    notification::Kind::NonBufferableTokensUsed(
                        tokens
                            .clone()
                            .into_iter()
                            .map(|token| token.into())
                            .collect(),
                    )
                }
                Kind::SolverAccountInsufficientBalance(required) => {
                    notification::Kind::SolverAccountInsufficientBalance(eth::Ether(*required))
                }
                Kind::DuplicatedSolutionId => notification::Kind::DuplicatedSolutionId,
            },
        }
    }
}

#[serde_as]
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Notification {
    #[serde_as(as = "Option<DisplayFromStr>")]
    auction_id: Option<i64>,
    solution_id: u64,
    kind: Kind,
}

#[serde_as]
#[derive(Debug, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    EmptySolution,
    ScoringFailed(ScoreKind),
    NonBufferableTokensUsed(BTreeSet<H160>),
    SolverAccountInsufficientBalance(U256),
    DuplicatedSolutionId,
}

#[serde_as]
#[derive(Debug, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ScoreKind {
    ZeroScore,
    ObjectiveValueNonPositive,
    SuccessProbabilityOutOfRange(f64),
    ScoreHigherThanObjective { score: U256, objective_value: U256 },
}
