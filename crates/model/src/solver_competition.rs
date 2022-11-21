use crate::{
    auction::AuctionId,
    order::OrderUid,
    u256_decimal::{self, DecimalU256},
};
use primitive_types::{H160, H256, U256};
use serde::{Deserialize, Serialize};
use serde_with::serde_as;
use std::collections::BTreeMap;

/// As a temporary measure the driver informs the api about per competition data that should be
/// stored in the database. This goes to the api through an unlisted and authenticated http endpoint
/// because we do not want the driver to have a database connection.
/// Once autopilot is handling the competition this will no longer be needed.
#[serde_as]
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct Request {
    pub auction: AuctionId,
    pub transaction: Transaction,
    pub competition: SolverCompetitionDB,
    pub executions: Vec<(OrderUid, Execution)>,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct Transaction {
    pub account: H160,
    pub nonce: u64,
}

#[serde_as]
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct Execution {
    pub reward: f64,
    #[serde_as(as = "Option<DecimalU256>")]
    pub surplus_fee: Option<U256>,
}

/// Stored directly in the database and turned into SolverCompetitionAPI for the
/// `/solver_competition` endpoint.
#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SolverCompetitionDB {
    pub gas_price: f64,
    pub auction_start_block: u64,
    pub liquidity_collected_block: u64,
    pub competition_simulation_block: u64,
    pub auction: CompetitionAuction,
    pub solutions: Vec<SolverSettlement>,
}

/// Returned by the `/solver_competition` endpoint.
#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SolverCompetitionAPI {
    pub auction_id: AuctionId,
    pub transaction_hash: Option<H256>,
    #[serde(flatten)]
    pub common: SolverCompetitionDB,
}

#[serde_as]
#[derive(Clone, Debug, Default, Deserialize, Serialize, Eq, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CompetitionAuction {
    pub orders: Vec<OrderUid>,
    #[serde_as(as = "BTreeMap<_, DecimalU256>")]
    pub prices: BTreeMap<H160, U256>,
}

#[serde_as]
#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SolverSettlement {
    pub solver: String,
    pub objective: Objective,
    #[serde_as(as = "BTreeMap<_, DecimalU256>")]
    pub clearing_prices: BTreeMap<H160, U256>,
    pub orders: Vec<Order>,
    #[serde(with = "crate::bytes_hex")]
    pub call_data: Vec<u8>,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Objective {
    pub total: f64,
    pub surplus: f64,
    pub fees: f64,
    pub cost: f64,
    pub gas: u64,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize, Eq, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Order {
    pub id: OrderUid,
    #[serde(with = "u256_decimal")]
    pub executed_amount: U256,
}

#[cfg(test)]
mod tests {
    use super::*;
    use maplit::btreemap;

    #[test]
    fn serialize() {
        let correct = serde_json::json!({
            "auctionId": 0,
            "gasPrice": 1.0f64,
            "auctionStartBlock": 13u64,
            "liquidityCollectedBlock": 14u64,
            "competitionSimulationBlock": 15u64,
            "transactionHash": "0x1111111111111111111111111111111111111111111111111111111111111111",
            "auction": {
                "orders": [
                    "0x1111111111111111111111111111111111111111111111111111111111111111\
                       1111111111111111111111111111111111111111\
                       11111111",
                    "0x2222222222222222222222222222222222222222222222222222222222222222\
                       2222222222222222222222222222222222222222\
                       22222222",
                    "0x3333333333333333333333333333333333333333333333333333333333333333\
                       3333333333333333333333333333333333333333\
                       33333333",
                ],
                "prices": {
                    "0x1111111111111111111111111111111111111111": "1000",
                    "0x2222222222222222222222222222222222222222": "2000",
                    "0x3333333333333333333333333333333333333333": "3000",
                },
            },
            "solutions": [
                {
                    "solver": "2",
                    "objective": {
                        "total": 3.0f64,
                        "surplus": 4.0f64,
                        "fees": 5.0f64,
                        "cost": 6.0f64,
                        "gas": 7u64,
                    },
                    "clearingPrices": {
                        "0x2222222222222222222222222222222222222222": "8",
                    },
                    "orders": [
                        {
                            "id": "0x3333333333333333333333333333333333333333333333333333333333333333\
                                     3333333333333333333333333333333333333333\
                                     33333333",
                            "executedAmount": "12",
                        }
                    ],
                    "callData": "0x13",
                },
            ],
        });

        let orig = SolverCompetitionAPI {
            auction_id: 0,
            transaction_hash: Some(H256([0x11; 32])),
            common: SolverCompetitionDB {
                gas_price: 1.,
                auction_start_block: 13,
                liquidity_collected_block: 14,
                competition_simulation_block: 15,
                auction: CompetitionAuction {
                    orders: vec![
                        OrderUid([0x11; 56]),
                        OrderUid([0x22; 56]),
                        OrderUid([0x33; 56]),
                    ],
                    prices: btreemap! {
                        H160([0x11; 20]) => 1000.into(),
                        H160([0x22; 20]) => 2000.into(),
                        H160([0x33; 20]) => 3000.into(),
                    },
                },
                solutions: vec![SolverSettlement {
                    solver: "2".to_string(),
                    objective: Objective {
                        total: 3.,
                        surplus: 4.,
                        fees: 5.,
                        cost: 6.,
                        gas: 7,
                    },
                    clearing_prices: btreemap! {
                        H160([0x22; 20]) => 8.into(),
                    },
                    orders: vec![Order {
                        id: OrderUid([0x33; 56]),
                        executed_amount: 12.into(),
                    }],
                    call_data: vec![0x13],
                }],
            },
        };

        let serialized = serde_json::to_value(&orig).unwrap();
        assert_eq!(correct, serialized);
        let deserialized: SolverCompetitionAPI = serde_json::from_value(correct).unwrap();
        assert_eq!(orig, deserialized);
    }
}
