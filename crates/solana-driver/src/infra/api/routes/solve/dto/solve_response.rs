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
    /// `auction` supplies each order's side and tokens.
    pub fn new(solutions: Vec<domain::Solution>, auction: &domain::Auction) -> Self {
        let auction_orders: HashMap<OrderUid, &domain::Order> = auction
            .orders
            .iter()
            .map(|order| (order.uid, order))
            .collect();
        Self {
            solutions: solutions
                .into_iter()
                .map(|solution| Solution::new(solution, &auction_orders))
                .collect(),
        }
    }
}

impl Solution {
    fn new(solution: domain::Solution, auction_orders: &HashMap<OrderUid, &domain::Order>) -> Self {
        let domain::Solution {
            id,
            prices,
            trades,
            solver,
            ..
        } = solution;
        let orders = trades
            .into_iter()
            .map(|trade| {
                let order = auction_orders.get(&trade.order_uid).expect(
                    "trade uid is known by construction: Solutions::into_domain rejects unknown \
                     uids",
                );
                // The engine reports one executed amount, on the order's own side, plus uniform
                // clearing prices per mint. Derive the counterpart leg from the prices so the
                // autopilot sees a real trade on both sides instead of a zero placeholder.
                let price_sell = prices
                    .get(&order.sell_token)
                    .expect("engine reports a clearing price for every traded sell mint");
                let price_buy = prices
                    .get(&order.buy_token)
                    .expect("engine reports a clearing price for every traded buy mint");
                let (executed_sell, executed_buy) = match order.side {
                    domain::Side::Sell => {
                        let executed_buy = trade
                            .executed_amount
                            .checked_mul(*price_sell)
                            .and_then(|v| v.checked_div(*price_buy))
                            .expect("clearing prices yield a valid counterpart amount");
                        (trade.executed_amount, executed_buy)
                    }
                    domain::Side::Buy => {
                        let executed_sell = trade
                            .executed_amount
                            .checked_mul(*price_buy)
                            .and_then(|v| v.checked_div(*price_sell))
                            .expect("clearing prices yield a valid counterpart amount");
                        (executed_sell, trade.executed_amount)
                    }
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
            solution_id: id,
            // TODO: the driver stubs the score to 0 until surplus math is done, which needs
            // native price functionality.
            score: 0,
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

    /// A sell order fills `executedSell` and derives `executedBuy` from the
    /// clearing prices. A buy order does the reverse.
    #[test]
    fn new_derives_the_counterpart_leg_from_clearing_prices() {
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
            // Sell mint prices at the amount bought, buy mint at the amount sold.
            prices: HashMap::from([
                (Pubkey::new_from_array([0x33; 32]), 200),
                (Pubkey::new_from_array([0x44; 32]), 100),
            ]),
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
        // Sell order: 100 sold, 100 * 200 / 100 = 200 bought.
        assert_eq!(solution.orders[&OrderUid([0x11; 32])].executed_sell, 100);
        assert_eq!(solution.orders[&OrderUid([0x11; 32])].executed_buy, 200);
        // Buy order: 200 bought, 200 * 100 / 200 = 100 sold.
        assert_eq!(solution.orders[&OrderUid([0x22; 32])].executed_sell, 100);
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
