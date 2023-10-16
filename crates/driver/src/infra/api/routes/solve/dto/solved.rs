use {
    crate::{
        domain::{competition, competition::order, eth},
        infra::Solver,
        util::serialize,
    },
    serde::Serialize,
    serde_with::serde_as,
    std::collections::HashMap,
};

impl Solved {
    pub fn new(solved: Option<competition::Solved>, solver: &Solver) -> Self {
        let solutions = solved
            .into_iter()
            .map(|solved| Solution::new(0, solved, solver))
            .collect();
        Self { solutions }
    }
}

#[serde_as]
#[derive(Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Solved {
    solutions: Vec<Solution>,
}

impl Solution {
    pub fn new(solution_id: u64, solved: competition::Solved, solver: &Solver) -> Self {
        Self {
            solution_id,
            score: solved.score.0.get(),
            submission_address: solver.address().into(),
            orders: solved
                .orders
                .into_iter()
                .map(|(order_id, amounts)| {
                    (
                        order_id.into(),
                        OrderAmounts {
                            in_amount: amounts.in_amount.into(),
                            out_amount: amounts.out_amount.into(),
                        },
                    )
                })
                .collect(),
        }
    }
}

#[serde_as]
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OrderAmounts {
    #[serde_as(as = "serialize::U256")]
    pub in_amount: eth::U256,
    #[serde_as(as = "serialize::U256")]
    pub out_amount: eth::U256,
}

type OrderId = [u8; order::UID_LEN];

#[serde_as]
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Solution {
    /// Unique ID of the solution (per driver competition), used to identify it
    /// in subsequent requests (reveal, settle).
    #[serde_as(as = "serde_with::DisplayFromStr")]
    solution_id: u64,
    #[serde_as(as = "serialize::U256")]
    score: eth::U256,
    submission_address: eth::H160,
    #[serde_as(as = "HashMap<serialize::Hex, _>")]
    orders: HashMap<OrderId, OrderAmounts>,
}
