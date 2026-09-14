mod healthz;
mod quote;
mod settle;
mod solve;

use serde::{Deserialize, Serialize};

pub use self::{healthz::healthz, solve::AuctionError};
pub(crate) use self::{quote::quote, settle::settle, solve::solve};

/// Whether an order sells or buys an exact amount. Shared by the solve and
/// quote wire requests, where it serializes as `"sell"` or `"buy"`.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub(crate) enum Kind {
    Sell,
    Buy,
}

impl From<Kind> for crate::domain::Side {
    fn from(kind: Kind) -> Self {
        match kind {
            Kind::Sell => Self::Sell,
            Kind::Buy => Self::Buy,
        }
    }
}
