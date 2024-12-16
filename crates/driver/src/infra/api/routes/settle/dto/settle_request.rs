use {super::super::super::deserialize_solution_id, serde::Deserialize, serde_with::serde_as};

#[serde_as]
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SettleRequest {
    /// Unique ID of the solution (per driver competition), to settle.
    #[serde(deserialize_with = "deserialize_solution_id")]
    pub solution_id: u64,
    /// The last block number in which the solution TX can be included
    pub submission_deadline_latest_block: u64,
    /// Auction ID in which this solution is competing.
    #[serde_as(as = "Option<serde_with::DisplayFromStr>")]
    pub auction_id: Option<i64>,
}
