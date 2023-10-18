use {
    crate::domain::{auction, notification},
    serde::Deserialize,
    serde_with::{serde_as, DisplayFromStr},
};

impl Notification {
    /// Converts a data transfer object into its domain object representation.
    pub fn to_domain(&self) -> notification::Notification {
        notification::Notification {
            auction_id: match self.auction_id {
                Some(id) => auction::Id::Solve(id),
                None => auction::Id::Quote,
            },
            kind: match self.kind {
                Kind::EmptySolution => notification::Kind::EmptySolution,
                Kind::PriceViolation => notification::Kind::PriceViolation,
                Kind::ScoringFailed => notification::Kind::ScoringFailed,
                Kind::UntrustedInternalization => notification::Kind::UntrustedInternalization,
                Kind::InsufficientBalance => notification::Kind::InsufficientBalance,
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
    PriceViolation,
    ScoringFailed,
    UntrustedInternalization,
    InsufficientBalance,
    // todo
}
