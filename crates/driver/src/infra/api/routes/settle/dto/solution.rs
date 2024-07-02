use {serde::Deserialize, serde_with::serde_as};

#[serde_as]
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Solution {
    #[allow(dead_code)]
    /// Unique ID of the solution (per driver competition), to settle.
    #[serde_as(as = "serde_with::DisplayFromStr")]
    solution_id: u64,
    #[allow(dead_code)]
    /// The last block number in which the solution TX can be included
    #[serde_as(as = "serde_with::DisplayFromStr")]
    pub block_deadline: u64,
}
