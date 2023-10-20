use {
    crate::{
        domain::{competition::auction, eth},
        infra::notify,
    },
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
                notify::Kind::EmptySolution(solution) => Kind::EmptySolution(solution.0),
                notify::Kind::ScoringFailed => Kind::ScoringFailed,
                notify::Kind::NonBufferableTokensUsed(tokens) => Kind::NonBufferableTokensUsed(
                    tokens.into_iter().map(|token| token.0 .0).collect(),
                ),
                notify::Kind::SolverAccountInsufficientBalance(required) => {
                    Kind::SolverAccountInsufficientBalance(required.0)
                }
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
    kind: Kind,
}

#[serde_as]
#[derive(Debug, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    EmptySolution(u64),
    ScoringFailed,
    NonBufferableTokensUsed(BTreeSet<H160>),
    SolverAccountInsufficientBalance(U256),
    Settled(Settlement),
}

#[serde_as]
#[derive(Debug, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Settlement {
    Success(eth::H256),
    Revert(eth::H256),
    Fail,
}
