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
            kind: match &self.kind {
                Kind::EmptySolution => notification::Kind::EmptySolution,
                Kind::ScoringFailed(ScoreKind::ObjectiveValueNonPositive(value)) => {
                    notification::Kind::ScoringFailed(
                        notification::ScoreKind::ObjectiveValueNonPositive(*value),
                    )
                }
                Kind::ScoringFailed(ScoreKind::ScoreHigherThanObjective(score)) => {
                    notification::Kind::ScoringFailed(
                        notification::ScoreKind::ScoreHigherThanObjective((*score).into()),
                    )
                }
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
}

#[serde_as]
#[derive(Debug, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ScoreKind {
    SuccessProbabilityOutOfRange(f64),
    ObjectiveValueNonPositive(f64),
    ScoreHigherThanObjective(U256),
}
