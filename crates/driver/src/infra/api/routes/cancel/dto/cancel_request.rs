use {serde::Deserialize, serde_with::serde_as};

/// Asks the driver to drop a settlement the solver no longer wants executed.
/// The solution is named by the same pair the autopilot used to request the
/// settlement, which is what the solver knows it by.
#[serde_as]
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CancelRequest {
    /// Unique ID of the solution (per driver competition) to withdraw.
    pub solution_id: u64,
    /// Auction ID in which this solution is competing.
    #[serde_as(as = "serde_with::DisplayFromStr")]
    pub auction_id: i64,
}
