use super::auction;

/// The notification about important events happened in driver, that solvers
/// need to know about.
#[derive(Debug)]
pub struct Notification {
    pub auction_id: auction::Id,
    pub kind: Kind,
}

/// All types of notifications solvers can be informed about.
#[derive(Debug)]
pub enum Kind {
    EmptySolution,
    PriceViolation,
    ScoringFailed,
    UntrustedInternalization,
    InsufficientBalance,
    // .. todo
}
