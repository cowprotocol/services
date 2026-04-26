use {
    crate::{
        domain::{competition, competition::order},
        infra::Solver,
    },
    eth_domain_types as eth,
    serde::{Deserialize, Serialize},
    serde_with::serde_as,
    std::collections::HashMap,
    winner_selection::solution_hash::{HashableOrder, HashableSolution},
};

impl SolveResponse {
    pub fn new(solved: Vec<competition::Solved>, solver: &Solver) -> Self {
        let solutions = solved
            .into_iter()
            .map(|solved| Solution::new(solved.id.get(), solved, solver))
            .collect();
        Self { solutions }
    }
}

#[serde_as]
#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SolveResponse {
    pub solutions: Vec<Solution>,
}

impl Solution {
    pub fn new(solution_id: u64, solved: competition::Solved, solver: &Solver) -> Self {
        Self {
            solution_id,
            score: solved.score.0,
            submission_address: solver.address(),
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
                            limit_sell: amounts.sell.amount.into(),
                            buy_token: amounts.buy.token.into(),
                            limit_buy: amounts.buy.amount.into(),
                            executed_sell: amounts.executed_sell.into(),
                            executed_buy: amounts.executed_buy.into(),
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

pub(crate) type OrderId = [u8; order::UID_LEN];

#[serde_as]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Solution {
    /// Unique ID of the solution (per driver competition), used to identify it
    /// in subsequent requests (reveal, settle).
    pub solution_id: u64,
    pub submission_address: eth::Address,
    #[serde_as(as = "serde_ext::U256")]
    pub score: eth::U256,
    #[serde_as(as = "HashMap<serde_ext::Hex, _>")]
    pub orders: HashMap<OrderId, TradedOrder>,
    #[serde_as(as = "HashMap<_, serde_ext::U256>")]
    pub clearing_prices: HashMap<eth::Address, eth::U256>,
}

#[serde_as]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TradedOrder {
    pub side: Side,
    pub sell_token: eth::Address,
    pub buy_token: eth::Address,
    #[serde_as(as = "serde_ext::U256")]
    /// Sell limit order amount.
    pub limit_sell: eth::U256,
    #[serde_as(as = "serde_ext::U256")]
    /// Buy limit order amount.
    pub limit_buy: eth::U256,
    /// The effective amount that left the user's wallet including all fees.
    #[serde_as(as = "serde_ext::U256")]
    pub executed_sell: eth::U256,
    /// The effective amount the user received after all fees.
    #[serde_as(as = "serde_ext::U256")]
    pub executed_buy: eth::U256,
}

#[serde_as]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Side {
    Buy,
    Sell,
}

impl Solution {
    /// Builds the canonical view used by `winner_selection::solution_hash`.
    pub fn as_hashable(&self) -> HashableSolution<'_> {
        HashableSolution {
            solution_id: self.solution_id,
            solver_address: self.submission_address.as_slice(),
            orders: self
                .orders
                .iter()
                .map(|(uid, order)| (uid.as_slice(), order.as_hashable()))
                .collect(),
            prices: self
                .clearing_prices
                .iter()
                .map(|(token, price)| (token.as_slice(), price.to_be_bytes()))
                .collect(),
        }
    }
}

impl TradedOrder {
    fn as_hashable(&self) -> HashableOrder<'_> {
        HashableOrder {
            side: match self.side {
                Side::Buy => 0,
                Side::Sell => 1,
            },
            sell_token: self.sell_token.as_slice(),
            sell_amount: self.limit_sell.to_be_bytes(),
            buy_token: self.buy_token.as_slice(),
            buy_amount: self.limit_buy.to_be_bytes(),
            executed_sell: self.executed_sell.to_be_bytes(),
            executed_buy: self.executed_buy.to_be_bytes(),
        }
    }
}
