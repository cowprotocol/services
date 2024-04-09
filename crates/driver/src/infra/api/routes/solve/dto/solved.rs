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
            score: solved.score.0,
            submission_address: solver.address().into(),
            orders: solved
                .trades
                .into_iter()
                .map(|(order_id, amounts)| {
                    (
                        order_id.into(),
                        TradedAmounts {
                            sell_amount: amounts.sell.into(),
                            buy_amount: amounts.buy.into(),
                        },
                    )
                })
                .collect(),
            clearing_prices: solved
                .prices
                .into_iter()
                .map(|(k, v)| (k.into(), v.into()))
                .collect(),
        }
    }
}

#[serde_as]
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TradedAmounts {
    /// The effective amount that left the user's wallet including all fees.
    #[serde_as(as = "serialize::U256")]
    pub sell_amount: eth::U256,
    /// The effective amount the user received after all fees.
    #[serde_as(as = "serialize::U256")]
    pub buy_amount: eth::U256,
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
    orders: HashMap<OrderId, TradedAmounts>,
    #[serde_as(as = "HashMap<_, serialize::U256>")]
    clearing_prices: HashMap<eth::H160, eth::U256>,
}
