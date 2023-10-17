use crate::domain::competition;

/// A notification is sent to the solvers in case a solution failed validation.
#[derive(Debug)]
pub struct Notification {
    pub auction_id: Option<competition::auction::Id>,
    pub kind: Kind,
}

#[derive(Debug)]
pub enum Kind {
    /// The solution doesn't contain any user orders.
    EmptySolution, // NoUserOrders,
    /// The solution violated a price constraint (ie. max deviation to external
    /// price vector)
    PriceViolation,
    /// No valid score could be computed for the solution.
    ScoringFailed,
    // /// The solution didn't pass simulation. Includes all data needed to
    // /// re-create simulation locally
    // SimulationFailure(TransactionWithError),
    // /// Objective value is too low.
    // ObjectiveValueNonPositive,
    // /// The solution doesn't have a positive score. Currently this can happen
    // /// only if the objective value is negative.
    // NonPositiveScore,
    // /// The solution has a score that is too high. This can happen if the
    // /// score is higher than the maximum score (surplus + fees)
    // #[serde(rename_all = "camelCase")]
    // TooHighScore {
    //     #[serde_as(as = "HexOrDecimalU256")]
    //     surplus: U256,
    //     #[serde_as(as = "HexOrDecimalU256")]
    //     fees: U256,
    //     #[serde_as(as = "HexOrDecimalU256")]
    //     max_score: U256,
    //     #[serde_as(as = "HexOrDecimalU256")]
    //     submitted_score: U256,
    // },
}
