use {
    crate::{
        domain::competition::{self, solution},
        util::serialize,
    },
    serde::Serialize,
    serde_with::serde_as,
};

impl Solution {
    pub fn from_domain(id: solution::Id, score: competition::Score) -> Self {
        Self {
            id: id.to_bytes(),
            score: score.into(),
        }
    }
}

#[serde_as]
#[derive(Debug, Serialize)]
pub struct Solution {
    #[serde_as(as = "serialize::Hex")]
    id: [u8; 4],
    score: f64,
}
