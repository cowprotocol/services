//! Outbound `/solve` response: the solutions the driver proposes to the
//! autopilot.
//!
//! This is the driver's own mirror of `autopilot-svm`'s
//! `infra/driver/dto.rs::SolveResponse`. The pinned-literal test below keeps
//! the wire format in sync.

use {
    crate::domain::{self, order_uid::OrderUid},
    serde::{Deserialize, Serialize},
    serde_with::{DisplayFromStr, serde_as},
    solana_sdk::pubkey::Pubkey,
    std::collections::HashMap,
};

/// The driver's `/solve` answer.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SolveResponse {
    solutions: Vec<Solution>,
}

/// One proposed solution: enough to rank it, detect order overlap between
/// winners, and attribute its settlement on chain.
#[serde_as]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Solution {
    solution_id: u64,
    /// The keypair the driver settles with, the on-chain solver identity.
    #[serde_as(as = "DisplayFromStr")]
    solver: Pubkey,
    /// Executed amounts per filled order.
    #[serde_as(as = "HashMap<DisplayFromStr, _>")]
    orders: HashMap<OrderUid, TradedAmounts>,
}

/// What a solution executes for one order.
#[serde_as]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TradedAmounts {
    #[serde_as(as = "DisplayFromStr")]
    executed_sell: u64,
    #[serde_as(as = "DisplayFromStr")]
    executed_buy: u64,
    /// Units the driver took from the fill, in the buy token of a sell order
    /// and the sell token of a buy order: the execution cost, the solver fee
    /// and the protocol fee.
    #[serde_as(as = "DisplayFromStr")]
    fee: u64,
}

impl SolveResponse {
    /// Build the wire response from the driver's domain solutions.
    pub fn new(solutions: Vec<domain::Solution>) -> Self {
        Self {
            solutions: solutions.into_iter().map(Solution::new).collect(),
        }
    }
}

impl Solution {
    fn new(solution: domain::Solution) -> Self {
        let domain::Solution {
            id, trades, solver, ..
        } = solution;
        let orders = trades
            .into_iter()
            .map(|trade| {
                (
                    trade.order_uid,
                    TradedAmounts {
                        executed_sell: trade.executed_sell,
                        executed_buy: trade.executed_buy,
                        fee: trade.fee(),
                    },
                )
            })
            .collect();
        Self {
            solution_id: id,
            solver,
            orders,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Pins the wire format against the same literals as
    /// `autopilot-svm/src/infra/driver/dto.rs::tests`.
    #[test]
    fn solve_response_pins_the_wire_format() {
        let solve = SolveResponse {
            solutions: vec![Solution {
                solution_id: 3,
                solver: Pubkey::new_from_array([0x22; 32]),
                orders: HashMap::from([(
                    OrderUid([0x11; 32]),
                    TradedAmounts {
                        executed_sell: 100,
                        executed_buy: 200,
                        fee: 6,
                    },
                )]),
            }],
        };
        let expected = serde_json::json!({
            "solutions": [{
                "solutionId": 3,
                "solver": "3JF3sEqM796hk5WFqA6EtmEwJQ9quALszsfJyvXNQKy3",
                "orders": {
                    "0x1111111111111111111111111111111111111111111111111111111111111111": {
                        "executedSell": "100",
                        "executedBuy": "200",
                        "fee": "6"
                    }
                }
            }]
        });
        assert_eq!(serde_json::to_value(&solve).unwrap(), expected);
    }

    /// A trade's fee is everything the driver took from its fill: the
    /// execution cost, the solver fee and the protocol fee.
    #[test]
    fn a_trades_fee_adds_up_what_the_driver_took() {
        let solution = Solution::new(domain::Solution {
            id: 3,
            solver: Pubkey::new_from_array([0x22; 32]),
            prices: HashMap::new(),
            trades: vec![domain::Trade {
                order_uid: OrderUid([0x11; 32]),
                executed_sell: 100,
                executed_buy: 200,
                solver_fee: 2,
                protocol_fee: 1,
                execution_cost: 3,
            }],
            interactions: vec![],
            cu_estimate: None,
        });
        assert_eq!(solution.orders[&OrderUid([0x11; 32])].fee, 6);
    }
}
