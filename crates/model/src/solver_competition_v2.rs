use {
    crate::{auction::AuctionId, order::OrderUid},
    number::serialization::HexOrDecimalU256,
    primitive_types::{H160, H256, U256},
    serde::{Deserialize, Serialize},
    serde_with::serde_as,
    std::collections::BTreeMap,
};

#[serde_as]
#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Response {
    pub auction_id: AuctionId,
    pub auction_start_block: i64,
    pub transaction_hashes: Vec<H256>,
    #[serde_as(as = "BTreeMap<_, HexOrDecimalU256>")]
    pub reference_scores: BTreeMap<H160, U256>,
    pub auction: Auction,
    pub solutions: Vec<Solution>,
}

#[serde_as]
#[derive(Clone, Debug, Default, Deserialize, Serialize, Eq, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Auction {
    pub orders: Vec<OrderUid>,
    #[serde_as(as = "BTreeMap<_, HexOrDecimalU256>")]
    pub prices: BTreeMap<H160, U256>,
}

#[serde_as]
#[derive(Clone, Default, Deserialize, Serialize, PartialEq, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Solution {
    pub solver_address: H160,
    #[serde_as(as = "HexOrDecimalU256")]
    pub score: U256,
    pub ranking: i64,
    #[serde_as(as = "BTreeMap<_, HexOrDecimalU256>")]
    pub clearing_prices: BTreeMap<H160, U256>,
    pub orders: Vec<Order>,
    pub is_winner: bool,
    pub filtered_out: bool,
    pub tx_hash: Option<H256>,
    #[serde_as(as = "Option<HexOrDecimalU256>")]
    pub reference_score: Option<U256>,
}

#[serde_as]
#[derive(Clone, Debug, Deserialize, Serialize, Eq, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Order {
    pub id: OrderUid,
    /// The effective amount that left the user's wallet including all fees.
    #[serde_as(as = "HexOrDecimalU256")]
    pub sell_amount: U256,
    /// The effective amount the user received after all fees.
    #[serde_as(as = "HexOrDecimalU256")]
    pub buy_amount: U256,
    /// The buy token address.
    pub buy_token: H160,
    /// The sell token address.
    pub sell_token: H160,
}

#[cfg(test)]
mod tests {
    use {super::*, maplit::btreemap, testlib::assert_json_matches};

    #[test]
    fn serialize() {
        let correct = serde_json::json!({
            "auctionId": 0,
            "auctionStartBlock": 13u64,
            "transactionHashes": ["0x3333333333333333333333333333333333333333333333333333333333333333"],
            "referenceScores": {
                "0x2222222222222222222222222222222222222222": "0",
            },
            "auction": {
                "orders": [
                    "0x1111111111111111111111111111111111111111111111111111111111111111\
                       111111111111111111111111111111111111111111111111",
                ],
                "prices": {
                    "0x2222222222222222222222222222222222222222": "2000",
                },
            },
            "solutions": [
                {
                    "solverAddress": "0x2222222222222222222222222222222222222222",
                    "score": "123",
                    "ranking": 1,
                    "clearingPrices": {
                        "0x2222222222222222222222222222222222222222": "8",
                    },
                    "orders": [
                        {
                            "id": "0x1111111111111111111111111111111111111111111111111111111111111111\
                                     111111111111111111111111111111111111111111111111",
                            "sellAmount": "12",
                            "buyAmount": "13",
                            "buyToken": "0x2222222222222222222222222222222222222222",
                            "sellToken": "0x2222222222222222222222222222222222222222"
                        },
                    ],
                    "referenceScore": "10",
                    "txHash": "0x3333333333333333333333333333333333333333333333333333333333333333",
                    "isWinner": true,
                    "filteredOut": false,
                },
            ],
        });

        let solver = H160([0x22; 20]);
        let tx = H256([0x33; 32]);

        let orig = Response {
            auction_id: 0,
            auction_start_block: 13,
            transaction_hashes: vec![tx],
            reference_scores: btreemap! {
                solver => 0.into()
            },
            auction: Auction {
                orders: vec![OrderUid([0x11; 56])],
                prices: btreemap! {
                    H160([0x22; 20]) => 2000.into(),
                },
            },
            solutions: vec![Solution {
                solver_address: solver,
                score: 123.into(),
                ranking: 1,
                clearing_prices: btreemap! {
                    H160([0x22; 20]) => 8.into(),
                },
                orders: vec![Order {
                    id: OrderUid([0x11; 56]),
                    sell_amount: 12.into(),
                    buy_amount: 13.into(),
                    buy_token: H160([0x22; 20]),
                    sell_token: H160([0x22; 20]),
                }],
                is_winner: true,
                filtered_out: false,
                tx_hash: Some(tx),
                reference_score: Some(10.into()),
            }],
        };

        let serialized = serde_json::to_value(&orig).unwrap();
        assert_json_matches!(correct, serialized);
        let deserialized: Response = serde_json::from_value(correct).unwrap();
        assert_eq!(orig, deserialized);
    }
}
