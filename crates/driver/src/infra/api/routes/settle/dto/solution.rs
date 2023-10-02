use {serde::Deserialize, serde_with::serde_as};

#[serde_as]
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Solution {
    #[allow(dead_code)]
    /// Unique ID of the solution to settle.
    solution_id: u64,
}
