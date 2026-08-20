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
    /// Total surplus in lamports, decimal string on the wire.
    #[serde_as(as = "DisplayFromStr")]
    score: u64,
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
}

impl SolveResponse {
    /// Build the wire response from the driver's domain solutions.
    ///
    /// `auction` supplies each order's side.
    pub fn new(solutions: Vec<domain::Solution>, auction: &domain::Auction) -> Self {
        let sides: HashMap<OrderUid, domain::Side> = auction
            .orders
            .iter()
            .map(|order| (order.uid, order.side))
            .collect();
        Self {
            solutions: solutions
                .into_iter()
                .map(|solution| Solution::new(solution, &sides))
                .collect(),
        }
    }
}

impl Solution {
    fn new(solution: domain::Solution, sides: &HashMap<OrderUid, domain::Side>) -> Self {
        let orders = solution
            .trades
            .into_iter()
            .map(|trade| {
                // WARN: As of now, the engine implementation only reports one executed amount,
                // on the order's own side.  We fill that side and leave the other side at 0 as
                // a placeholder.
                //
                // TODO: the counterpart amount isn't on the engine wire yet, so the `0` below
                // is knowingly wrong. The autopilot must not persist it or compute over it as
                // if it were real. The driver will replace it once the engine wire carries both
                // legs (or clearing prices arrive).
                //
                // TODO: once the engine wire carries both trade legs, `domain::Trade` will hold
                // the side directly and this lookup (plus the `.expect`) goes
                // away entirely.
                let side = sides.get(&trade.order_uid).copied().expect(
                    "trade uid is known by construction: Solutions::into_domain rejects unknown \
                     uids",
                );
                let (executed_sell, executed_buy) = match side {
                    domain::Side::Sell => (trade.executed_amount, 0),
                    domain::Side::Buy => (0, trade.executed_amount),
                };
                (
                    trade.order_uid,
                    TradedAmounts {
                        executed_sell,
                        executed_buy,
                    },
                )
            })
            .collect();
        Self {
            solution_id: solution.id,
            // TODO: the driver stubs the score to 0 until surplus math is done, which needs both
            // executed amounts and native price functionality.
            score: 0,
            solver: solution.solver,
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
                score: 12_345,
                solver: Pubkey::new_from_array([0x22; 32]),
                orders: HashMap::from([(
                    OrderUid([0x11; 32]),
                    TradedAmounts {
                        executed_sell: 100,
                        executed_buy: 200,
                    },
                )]),
            }],
        };
        let expected = serde_json::json!({
            "solutions": [{
                "solutionId": 3,
                "score": "12345",
                "solver": "3JF3sEqM796hk5WFqA6EtmEwJQ9quALszsfJyvXNQKy3",
                "orders": {
                    "0x1111111111111111111111111111111111111111111111111111111111111111": {
                        "executedSell": "100",
                        "executedBuy": "200"
                    }
                }
            }]
        });
        assert_eq!(serde_json::to_value(&solve).unwrap(), expected);
    }

    /// A sell order fills `executedSell` and zero-fills `executedBuy`. A buy
    /// order does the reverse. The zero is the placeholder.
    ///
    /// TODO: temporary — obsolete once the engine wire carries both legs (or
    /// clearing prices arrive), at which point the zero placeholder goes away.
    #[test]
    fn new_fills_the_side_matching_amount() {
        let auction = domain::Auction {
            id: domain::auction::Id::new(1).unwrap(),
            orders: vec![
                domain::Order {
                    uid: OrderUid([0x11; 32]),
                    side: domain::Side::Sell,
                    ..order()
                },
                domain::Order {
                    uid: OrderUid([0x22; 32]),
                    side: domain::Side::Buy,
                    ..order()
                },
            ],
            deadline_slot: domain::Slot(1),
            deadline: chrono::Utc::now(),
        };
        let solutions = vec![domain::Solution {
            id: 0,
            solver: Pubkey::new_from_array([0x33; 32]),
            trades: vec![
                domain::Trade {
                    order_uid: OrderUid([0x11; 32]),
                    executed_amount: 100,
                },
                domain::Trade {
                    order_uid: OrderUid([0x22; 32]),
                    executed_amount: 200,
                },
            ],
            interactions: Vec::new(),
            address_lookup_tables: Vec::new(),
            cu_estimate: None,
        }];

        let response = SolveResponse::new(solutions, &auction);
        let solution = &response.solutions[0];
        assert_eq!(solution.score, 0, "score is stubbed to 0");
        assert_eq!(solution.orders[&OrderUid([0x11; 32])].executed_sell, 100);
        assert_eq!(solution.orders[&OrderUid([0x11; 32])].executed_buy, 0);
        assert_eq!(solution.orders[&OrderUid([0x22; 32])].executed_sell, 0);
        assert_eq!(solution.orders[&OrderUid([0x22; 32])].executed_buy, 200);
    }

    fn order() -> domain::Order {
        domain::Order {
            uid: OrderUid([0x11; 32]),
            owner: Pubkey::new_from_array([0x22; 32]),
            sell_token: Pubkey::new_from_array([0x33; 32]),
            buy_token: Pubkey::new_from_array([0x44; 32]),
            sell_token_account: Pubkey::new_from_array([0x55; 32]),
            buy_token_account: Pubkey::new_from_array([0x66; 32]),
            sell_amount: 1_000,
            buy_amount: 2_000,
            valid_to: 42,
            side: domain::Side::Sell,
            partially_fillable: false,
            order_pda: Pubkey::new_from_array([0x77; 32]),
        }
    }
}
