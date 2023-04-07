use {
    crate::domain::{
        competition::{self, solution},
        eth,
    },
    primitive_types::H160,
    serde::Serialize,
    serde_with::{serde_as, DisplayFromStr},
};

impl Solution {
    pub fn from_domain(
        id: solution::Id,
        score: competition::Score,
        participation_reward_address: eth::Address,
    ) -> Self {
        Self {
            id: id.into(),
            score: score.into(),
            participation_reward_address: participation_reward_address.into(),
        }
    }
}

#[serde_as]
#[derive(Debug, Serialize)]
pub struct Solution {
    #[serde_as(as = "DisplayFromStr")]
    id: u64,
    score: f64,
    participation_reward_address: H160,
}
