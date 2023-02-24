use {
    crate::domain::competition::{self, solution},
    serde::Serialize,
    serde_with::{serde_as, DisplayFromStr},
};

impl Solution {
    /// A None score indicates no solution was found.
    pub fn from_domain(id: solution::Id, score: Option<competition::Score>) -> Self {
        Self {
            id: id.into(),
            score: score.map(Into::into),
        }
    }
}

#[serde_as]
#[derive(Debug, Serialize)]
pub struct Solution {
    #[serde_as(as = "DisplayFromStr")]
    id: u64,
    score: Option<f64>,
}
