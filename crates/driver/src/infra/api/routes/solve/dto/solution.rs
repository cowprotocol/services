use {
    crate::domain::competition::{self, solution},
    primitive_types::H160,
    serde::Serialize,
    serde_with::{serde_as, DisplayFromStr},
};

impl Solution {
    pub fn from_domain(
        id: solution::Id,
        score: competition::Score,
        rewards: competition::Reward,
    ) -> Self {
        Self {
            id: id.into(),
            score: score.into(),
            reward: Reward {
                performance_address: rewards.performance_address.into(),
                participation_address: rewards.participation_address.into(),
            },
        }
    }
}

#[serde_as]
#[derive(Debug, Serialize)]
pub struct Reward {
    performance_address: H160,
    participation_address: H160,
}

#[serde_as]
#[derive(Debug, Serialize)]
pub struct Solution {
    #[serde_as(as = "DisplayFromStr")]
    id: u64,
    score: f64,
    reward: Reward,
}
