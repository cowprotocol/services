use {serde::Deserialize, serde_with::serde_as};

#[serde_as]
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Solution {
    /// Unique ID of the solution (per driver competition), to reveal.
    #[serde_as(as = "serde_with::DisplayFromStr")]
    pub solution_id: u64,
}
