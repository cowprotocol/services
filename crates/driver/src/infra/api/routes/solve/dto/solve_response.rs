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

impl SolveResponse {
    pub fn new(solved: Option<competition::Solved>, solver: &Solver) -> Self {
        let solutions = solved
            .into_iter()
            .map(|solved| Solution::new(solved.id.get(), solved, solver))
            .collect();
        Self { solutions }
    }
}

#[serde_as]
#[derive(Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SolveResponse {
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
                        TradedOrder {
                            side: match amounts.side {
                                order::Side::Buy => Side::Buy,
                                order::Side::Sell => Side::Sell,
                            },
                            sell_token: amounts.sell.token.into(),
                            limit_sell: amounts.sell.amount,
                            buy_token: amounts.buy.token.into(),
                            limit_buy: amounts.buy.amount,
                            executed_sell: amounts.executed_sell,
                            executed_buy: amounts.executed_buy,
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

type OrderId = [u8; order::UID_LEN];

#[serde_as]
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Solution {
    /// Unique ID of the solution (per driver competition), used to identify it
    /// in subsequent requests (reveal, settle).
    solution_id: u64,
    /// Solution quality score as a dimensionless value for ranking solutions.
    /// Not a token amount - represents objective function result.
    #[serde_as(as = "serialize::U256")]
    #[allow(clippy::disallowed_types)] // Dimensionless score
    score: eth::U256,
    submission_address: eth::H160,
    #[serde_as(as = "HashMap<serialize::Hex, _>")]
    orders: HashMap<OrderId, TradedOrder>,
    /// Clearing prices as dimensionless exchange rates between tokens.
    /// Maps token address → price ratio (not a token amount).
    #[serde_as(as = "HashMap<_, serialize::U256>")]
    #[allow(clippy::disallowed_types)] // Dimensionless ratios
    clearing_prices: HashMap<eth::H160, eth::U256>,
}

#[serde_as]
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TradedOrder {
    pub side: Side,
    pub sell_token: eth::H160,
    pub buy_token: eth::H160,
    /// Sell limit order amount.
    pub limit_sell: eth::TokenAmount,
    /// Buy limit order amount.
    pub limit_buy: eth::TokenAmount,
    /// The effective amount that left the user's wallet including all fees.
    pub executed_sell: eth::TokenAmount,
    /// The effective amount the user received after all fees.
    pub executed_buy: eth::TokenAmount,
}

#[serde_as]
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Side {
    Buy,
    Sell,
}
