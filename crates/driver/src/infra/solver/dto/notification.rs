use {
    crate::{domain::competition::auction, infra::notify},
    primitive_types::H160,
    serde::Serialize,
    serde_with::serde_as,
    std::collections::BTreeSet,
};

impl Notification {
    pub fn new(auction_id: Option<auction::Id>, kind: notify::Kind) -> Self {
        Self {
            auction_id: auction_id.as_ref().map(ToString::to_string),
            kind: match kind {
                notify::Kind::EmptySolution(solution) => Kind::EmptySolution(solution.0),
                notify::Kind::ScoringFailed => Kind::ScoringFailed,
                notify::Kind::NonBufferableTokensUsed(tokens) => Kind::NonBufferableTokensUsed(
                    tokens.into_iter().map(|token| token.0 .0).collect(),
                ),
                notify::Kind::SolverAccountInsufficientBalance => {
                    Kind::SolverAccountInsufficientBalance
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
    EmptySolution(u64),
    ScoringFailed,
    NonBufferableTokensUsed(BTreeSet<H160>),
    SolverAccountInsufficientBalance,
}
