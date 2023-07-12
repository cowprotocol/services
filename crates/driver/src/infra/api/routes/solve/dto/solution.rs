use {
    crate::{
        domain::{
            competition::{self, order},
            eth,
        },
        infra::Solver,
        util::serialize,
    },
    serde::Serialize,
    serde_with::serde_as,
};

impl Solution {
    pub fn new(reveal: competition::Solved, solver: &Solver) -> Self {
        Self {
            score: reveal.score.into(),
            submission_address: solver.address().into(),
            orders: reveal.orders.into_iter().map(Into::into).collect(),
        }
    }
}

#[serde_as]
#[derive(Debug, Serialize)]
pub struct Solution {
    #[serde_as(as = "serialize::U256")]
    score: eth::U256,
    submission_address: eth::H160,
    #[serde_as(as = "Vec<serialize::Hex>")]
    orders: Vec<[u8; order::UID_LEN]>,
}
