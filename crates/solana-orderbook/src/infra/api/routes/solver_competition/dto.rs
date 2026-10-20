//! Wire shape of the solver competition endpoints.

use {
    crate::infra::db::{self, CompetitionTrade},
    bigdecimal::BigDecimal,
    serde::Serialize,
    serde_with::{DisplayFromStr, serde_as},
    solana_sdk::{pubkey::Pubkey, signature::Signature},
    std::collections::{BTreeMap, HashMap},
};

/// One competition: the auction, every proposed solution, and the
/// settlements.
#[serde_as]
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Competition {
    pub auction_id: i64,
    /// The slot the auction was cut at.
    pub auction_start_slot: i64,
    /// The slot by which the winners had to settle.
    pub auction_deadline_slot: i64,
    /// The transactions that settled the auction.
    #[serde_as(as = "Vec<DisplayFromStr>")]
    pub transaction_signatures: Vec<Signature>,
    /// Per winning solver, the winners' total score with that solver's
    /// solutions removed.
    #[serde_as(as = "BTreeMap<DisplayFromStr, DisplayFromStr>")]
    pub reference_scores: BTreeMap<Pubkey, BigDecimal>,
    pub auction: Auction,
    /// Worst to best, the winner last.
    pub solutions: Vec<Solution>,
}

/// The auction the solutions competed on.
#[serde_as]
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Auction {
    /// The orders in the auction, as hex uids.
    pub orders: Vec<String>,
    /// The native price of every mint the auction trades, in lamports per
    /// 10^9 base units.
    #[serde_as(as = "BTreeMap<DisplayFromStr, DisplayFromStr>")]
    pub prices: BTreeMap<Pubkey, BigDecimal>,
}

/// One proposed solution.
#[serde_as]
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Solution {
    #[serde_as(as = "DisplayFromStr")]
    pub solver_address: Pubkey,
    #[serde_as(as = "DisplayFromStr")]
    pub score: BigDecimal,
    /// The solution's position in the competition, 1 the best.
    pub ranking: i64,
    pub orders: Vec<Order>,
    pub is_winner: bool,
    /// Excluded from the ranking by the fairness filter.
    pub filtered_out: bool,
    /// The transaction that executed the solution. Absent until the landed
    /// settlement is attributed to the solution.
    #[serde_as(as = "Option<DisplayFromStr>")]
    pub tx_signature: Option<Signature>,
    #[serde_as(as = "Option<DisplayFromStr>")]
    pub reference_score: Option<BigDecimal>,
}

/// One order execution a solution proposes.
#[serde_as]
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Order {
    pub id: String,
    #[serde_as(as = "DisplayFromStr")]
    pub sell_amount: BigDecimal,
    #[serde_as(as = "DisplayFromStr")]
    pub buy_amount: BigDecimal,
    #[serde_as(as = "DisplayFromStr")]
    pub sell_token: Pubkey,
    #[serde_as(as = "DisplayFromStr")]
    pub buy_token: Pubkey,
}

impl From<db::Competition> for Competition {
    fn from(competition: db::Competition) -> Self {
        let mut orders: HashMap<i64, Vec<Order>> = HashMap::new();
        for trade in competition.trades {
            orders
                .entry(trade.solution_uid)
                .or_default()
                .push(Order::from(trade));
        }
        let reference_scores = competition
            .solutions
            .iter()
            .filter_map(|solution| {
                Some((
                    Pubkey::new_from_array(solution.solver.0),
                    solution.reference_score.clone()?,
                ))
            })
            .collect();
        let solutions = competition
            .solutions
            .into_iter()
            .map(|solution| Solution {
                solver_address: Pubkey::new_from_array(solution.solver.0),
                score: solution.score,
                ranking: solution.ranking,
                orders: orders.remove(&solution.uid).unwrap_or_default(),
                is_winner: solution.is_winner,
                filtered_out: solution.filtered_out,
                tx_signature: solution
                    .tx_signature
                    .map(|signature| Signature::from(signature.0)),
                reference_score: solution.reference_score,
            })
            .collect();
        let auction = competition.auction;
        Self {
            auction_id: auction.id,
            auction_start_slot: auction.tip_slot,
            auction_deadline_slot: auction.deadline_slot,
            transaction_signatures: competition
                .tx_signatures
                .into_iter()
                .map(|signature| Signature::from(signature.0))
                .collect(),
            reference_scores,
            auction: Auction {
                orders: auction
                    .order_uids
                    .iter()
                    .map(|uid| const_hex::encode_prefixed(uid.0))
                    .collect(),
                prices: auction
                    .price_tokens
                    .into_iter()
                    .zip(auction.price_values)
                    .map(|(mint, price)| (Pubkey::new_from_array(mint.0), price))
                    .collect(),
            },
            solutions,
        }
    }
}

impl From<CompetitionTrade> for Order {
    fn from(trade: CompetitionTrade) -> Self {
        Self {
            id: const_hex::encode_prefixed(trade.order_uid.0),
            sell_amount: trade.executed_sell,
            buy_amount: trade.executed_buy,
            sell_token: Pubkey::new_from_array(trade.sell_token.0),
            buy_token: Pubkey::new_from_array(trade.buy_token.0),
        }
    }
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        crate::infra::db::{CompetitionAuction, CompetitionSolution},
        database::byte_array::ByteArray,
    };

    #[test]
    fn wire_format_is_stable() {
        let competition = Competition::from(db::Competition {
            auction: CompetitionAuction {
                id: 7,
                tip_slot: 75,
                deadline_slot: 100,
                order_uids: vec![ByteArray([0x11; 32])],
                price_tokens: vec![ByteArray([0x22; 32])],
                price_values: vec![2_000.into()],
            },
            solutions: vec![
                CompetitionSolution {
                    uid: 1,
                    ranking: 2,
                    solver: ByteArray([0x33; 32]),
                    is_winner: false,
                    filtered_out: true,
                    score: 5.into(),
                    reference_score: None,
                    tx_signature: None,
                },
                CompetitionSolution {
                    uid: 0,
                    ranking: 1,
                    solver: ByteArray([0x22; 32]),
                    is_winner: true,
                    filtered_out: false,
                    score: 123.into(),
                    reference_score: Some(10.into()),
                    tx_signature: Some(ByteArray([9; 64])),
                },
            ],
            trades: vec![CompetitionTrade {
                solution_uid: 0,
                order_uid: ByteArray([0x11; 32]),
                executed_sell: 12.into(),
                executed_buy: 13.into(),
                sell_token: ByteArray([0x22; 32]),
                buy_token: ByteArray([0x44; 32]),
            }],
            tx_signatures: vec![ByteArray([9; 64])],
        });
        let signature =
            "BUguQsv2ZuHus54HAFzjdJHzZBkygAjKhEeYwSG19tUfUyvvz3worsdQCdAXDNjakJHioSiyxhFiDJrm8XpSXRA";
        let winner = "3JF3sEqM796hk5WFqA6EtmEwJQ9quALszsfJyvXNQKy3";
        let loser = Pubkey::new_from_array([0x33; 32]).to_string();
        let buy_token = Pubkey::new_from_array([0x44; 32]).to_string();
        assert_eq!(
            serde_json::to_value(&competition).unwrap(),
            serde_json::json!({
                "auctionId": 7,
                "auctionStartSlot": 75,
                "auctionDeadlineSlot": 100,
                "transactionSignatures": [signature],
                "referenceScores": { winner: "10" },
                "auction": {
                    "orders": [format!("0x{}", "11".repeat(32))],
                    "prices": { winner: "2000" },
                },
                "solutions": [
                    {
                        "solverAddress": loser,
                        "score": "5",
                        "ranking": 2,
                        "orders": [],
                        "isWinner": false,
                        "filteredOut": true,
                        "txSignature": null,
                        "referenceScore": null,
                    },
                    {
                        "solverAddress": winner,
                        "score": "123",
                        "ranking": 1,
                        "orders": [{
                            "id": format!("0x{}", "11".repeat(32)),
                            "sellAmount": "12",
                            "buyAmount": "13",
                            "sellToken": winner,
                            "buyToken": buy_token,
                        }],
                        "isWinner": true,
                        "filteredOut": false,
                        "txSignature": signature,
                        "referenceScore": "10",
                    },
                ],
            })
        );
    }
}
