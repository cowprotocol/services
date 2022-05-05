use crate::{
    order::OrderUid,
    u256_decimal::{self, DecimalU256},
};
use primitive_types::{H160, H256, U256};
use serde::{Deserialize, Serialize};
use serde_with::serde_as;
use std::collections::HashMap;

#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SolverCompetitionResponse {
    pub gas_price: f64,
    pub liquidity_collected_block: u64,
    pub competition_simulation_block: u64,
    pub transaction_hash: Option<H256>,
    pub solutions: Vec<SolverSettlement>,
}

#[serde_as]
#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SolverSettlement {
    pub solver: String,
    pub objective: Objective,
    #[serde_as(as = "HashMap<_, DecimalU256>")]
    pub prices: HashMap<H160, U256>,
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

#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Order {
    pub id: OrderUid,
    #[serde(with = "u256_decimal")]
    pub executed_amount: U256,
}

#[cfg(test)]
mod tests {
    use super::*;
    use hex_literal::hex;

    #[test]
    fn serialize() {
        let correct = serde_json::json!({
            "transactionHash": "0x1111111111111111111111111111111111111111111111111111111111111111",
            "gasPrice": 1.0f64,
            "liquidityCollectedBlock": 14u64,
            "competitionSimulationBlock": 15u64,
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
                "prices": {
                    "0x2222222222222222222222222222222222222222": "8",
                },
                "orders": [
                    {
                    "id": "0x1111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111",
                    "executedAmount": "12"
                    }
                ],
                "callData": "0x13"
                }
            ]
        });
        let transaction_hash = H256(hex!(
            "1111111111111111111111111111111111111111111111111111111111111111"
        ));
        let order_id =OrderUid(hex!("1111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111"));
        let orig = SolverCompetitionResponse {
            gas_price: 1.,
            liquidity_collected_block: 14,
            competition_simulation_block: 15,
            transaction_hash: Some(transaction_hash),
            solutions: vec![SolverSettlement {
                solver: "2".to_string(),
                objective: Objective {
                    total: 3.,
                    surplus: 4.,
                    fees: 5.,
                    cost: 6.,
                    gas: 7,
                },
                prices: [(
                    H160(hex!("2222222222222222222222222222222222222222")),
                    8.into(),
                )]
                .into_iter()
                .collect(),
                orders: vec![Order {
                    id: order_id,
                    executed_amount: 12.into(),
                }],
                call_data: vec![0x13],
            }],
        };
        let serialized = serde_json::to_value(&orig).unwrap();
        assert_eq!(correct, serialized);
        let deserialized: SolverCompetitionResponse = serde_json::from_value(correct).unwrap();
        assert_eq!(orig, deserialized);
    }
}
