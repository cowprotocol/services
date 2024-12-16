use {super::super::super::deserialize_solution_id, serde::Deserialize, serde_with::serde_as};

#[serde_as]
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RevealRequest {
    /// Unique ID of the solution (per driver competition), to reveal.
    #[serde(deserialize_with = "deserialize_solution_id")]
    pub solution_id: u64,
    /// Auction ID in which the specified solution ID is competing.
    #[serde_as(as = "Option<serde_with::DisplayFromStr>")]
    pub auction_id: Option<i64>,
}
