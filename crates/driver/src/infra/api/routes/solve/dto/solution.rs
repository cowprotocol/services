use {
    crate::domain::competition::{self, solution::settlement},
    model::order::OrderUid,
    primitive_types::{H160, U256},
    serde::Serialize,
    serde_with::{serde_as, DisplayFromStr},
};

impl Solution {
    pub fn from_domain(
        id: settlement::Id,
        score: competition::Score,
        address: competition::SubmissionAddress,
        orders: Vec<OrderUid>,
    ) -> Self {
        Self {
            id: id.into(),
            score: score.into(),
            submission_address: address.0.into(),
            orders,
        }
    }
}

#[serde_as]
#[derive(Debug, Serialize)]
pub struct Solution {
    #[serde_as(as = "DisplayFromStr")]
    id: u64,
    score: U256,
    submission_address: H160,
    orders: Vec<OrderUid>,
}
