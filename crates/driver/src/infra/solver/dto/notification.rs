use {
    crate::{domain::competition, infra::notify},
    serde::Serialize,
    serde_with::serde_as,
};

impl Notification {
    pub fn new(auction_id: Option<competition::auction::Id>, kind: notify::Kind) -> Self {
        Self {
            auction_id: auction_id.as_ref().map(ToString::to_string),
            kind: match kind {
                notify::Kind::EmptySolution => Kind::EmptySolution,
                notify::Kind::ScoringFailed => Kind::ScoringFailed,
                notify::Kind::NonBufferableTokensUsed => Kind::UntrustedInternalization,
                notify::Kind::InsufficientBalance => Kind::InsufficientBalance,
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
    ScoringFailed,
    UntrustedInternalization,
    InsufficientBalance,
}
