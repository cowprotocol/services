//! Wire shape of the solver competition endpoints. Mirrors the EVM
//! `solver_competition_v2` response, with slots for blocks, signatures for
//! transaction hashes, and no clearing prices: the Solana solve wire carries
//! none.

use {
    crate::infra::db::CompetitionRows,
    bigdecimal::BigDecimal,
    serde::Serialize,
    serde_with::{DisplayFromStr, serde_as},
    solana_sdk::{pubkey::Pubkey, signature::Signature},
    std::collections::{BTreeMap, HashMap},
};

/// One competition on the wire.
#[serde_as]
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Competition {
    pub auction_id: i64,
    pub auction_start_slot: i64,
    pub auction_deadline_slot: i64,
    #[serde_as(as = "Vec<DisplayFromStr>")]
    pub transaction_signatures: Vec<Signature>,
    #[serde_as(as = "BTreeMap<DisplayFromStr, DisplayFromStr>")]
    pub reference_scores: BTreeMap<Pubkey, BigDecimal>,
    pub auction: Auction,
    /// Worst first, as on EVM.
    pub solutions: Vec<Solution>,
}

/// The orders and native prices the competition ran on.
#[serde_as]
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Auction {
    pub orders: Vec<String>,
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
    pub ranking: i64,
    pub orders: Vec<Order>,
    pub is_winner: bool,
    pub filtered_out: bool,
    /// The settlement of this solution. Absent until settlements are
    /// attributed to solutions.
    #[serde_as(as = "Option<DisplayFromStr>")]
    pub tx_signature: Option<Signature>,
    #[serde_as(as = "Option<DisplayFromStr>")]
    pub reference_score: Option<BigDecimal>,
}

/// One order execution of a solution.
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

fn order_uid(uid: [u8; 32]) -> String {
    format!("0x{}", const_hex::encode(uid))
}

impl From<CompetitionRows> for Competition {
    fn from(rows: CompetitionRows) -> Self {
        let mut orders = HashMap::<i64, Vec<Order>>::new();
        for trade in rows.trades {
            orders.entry(trade.solution_uid).or_default().push(Order {
                id: order_uid(trade.order_uid.0),
                sell_amount: trade.executed_sell,
                buy_amount: trade.executed_buy,
                sell_token: Pubkey::new_from_array(trade.sell_token.0),
                buy_token: Pubkey::new_from_array(trade.buy_token.0),
            });
        }
        let settlements: HashMap<_, _> = rows
            .settlements
            .iter()
            .filter_map(|s| Some((s.solution_uid?, Signature::from(s.tx_signature.0))))
            .collect();
        let reference_scores: BTreeMap<_, _> = rows
            .reference_scores
            .into_iter()
            .map(|row| (Pubkey::new_from_array(row.solver.0), row.reference_score))
            .collect();

        let mut solutions: Vec<_> = rows
            .solutions
            .into_iter()
            .map(|solution| {
                let solver = Pubkey::new_from_array(solution.solver.0);
                Solution {
                    solver_address: solver,
                    score: solution.score,
                    ranking: solution.uid + 1,
                    orders: orders.remove(&solution.uid).unwrap_or_default(),
                    is_winner: solution.is_winner,
                    filtered_out: solution.filtered_out,
                    tx_signature: settlements.get(&solution.uid).copied(),
                    reference_score: reference_scores.get(&solver).cloned(),
                }
            })
            .collect();
        solutions.sort_by_key(|s| std::cmp::Reverse(s.ranking));

        Self {
            auction_id: rows.auction.id,
            auction_start_slot: rows.auction.tip_slot,
            auction_deadline_slot: rows.auction.deadline_slot,
            transaction_signatures: rows
                .settlements
                .iter()
                .map(|s| Signature::from(s.tx_signature.0))
                .collect(),
            reference_scores,
            auction: Auction {
                orders: rows
                    .auction
                    .order_uids
                    .into_iter()
                    .map(|uid| order_uid(uid.0))
                    .collect(),
                prices: rows
                    .auction
                    .price_tokens
                    .into_iter()
                    .zip(rows.auction.price_values)
                    .map(|(token, price)| (Pubkey::new_from_array(token.0), price))
                    .collect(),
            },
            solutions,
        }
    }
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        crate::infra::db::{
            CompetitionAuctionRow,
            CompetitionSettlementRow,
            ProposedSolutionRow,
            ProposedTradeRow,
            ReferenceScoreRow,
        },
        database::byte_array::ByteArray,
    };

    #[test]
    fn wire_format_is_stable() {
        let solution = |uid, is_winner| ProposedSolutionRow {
            uid,
            solver: ByteArray([0x22; 32]),
            is_winner,
            filtered_out: false,
            score: 100.into(),
        };
        let competition = Competition::from(CompetitionRows {
            auction: CompetitionAuctionRow {
                id: 7,
                tip_slot: 90,
                deadline_slot: 100,
                order_uids: vec![ByteArray([0x11; 32])],
                price_tokens: vec![ByteArray([0x33; 32])],
                price_values: vec![2_000.into()],
            },
            solutions: vec![solution(0, true), solution(1, false)],
            trades: vec![ProposedTradeRow {
                solution_uid: 0,
                order_uid: ByteArray([0x11; 32]),
                executed_sell: 12.into(),
                executed_buy: 13.into(),
                sell_token: ByteArray([0x33; 32]),
                buy_token: ByteArray([0x44; 32]),
            }],
            settlements: vec![CompetitionSettlementRow {
                tx_signature: ByteArray([9; 64]),
                solution_uid: Some(0),
            }],
            reference_scores: vec![ReferenceScoreRow {
                solver: ByteArray([0x22; 32]),
                reference_score: 10.into(),
            }],
        });
        let solver = "3JF3sEqM796hk5WFqA6EtmEwJQ9quALszsfJyvXNQKy3";
        let signature = "BUguQsv2ZuHus54HAFzjdJHzZBkygAjKhEeYwSG19tUfUyvvz3worsdQCdAXDNjakJHioSiyxhFiDJrm8XpSXRA";
        let mint = |byte| Pubkey::new_from_array([byte; 32]).to_string();
        assert_eq!(
            serde_json::to_value(&competition).unwrap(),
            serde_json::json!({
                "auctionId": 7,
                "auctionStartSlot": 90,
                "auctionDeadlineSlot": 100,
                "transactionSignatures": [signature],
                "referenceScores": { solver: "10" },
                "auction": {
                    "orders": [format!("0x{}", "11".repeat(32))],
                    "prices": { mint(0x33): "2000" },
                },
                "solutions": [
                    {
                        "solverAddress": solver,
                        "score": "100",
                        "ranking": 2,
                        "orders": [],
                        "isWinner": false,
                        "filteredOut": false,
                        "txSignature": null,
                        "referenceScore": "10",
                    },
                    {
                        "solverAddress": solver,
                        "score": "100",
                        "ranking": 1,
                        "orders": [{
                            "id": format!("0x{}", "11".repeat(32)),
                            "sellAmount": "12",
                            "buyAmount": "13",
                            "sellToken": mint(0x33),
                            "buyToken": mint(0x44),
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
