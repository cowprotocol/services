//! Contains the Trade type as described by the specification with serialization as described by the openapi documentation.

use crate::order::OrderUid;
use num_bigint::BigUint;
use primitive_types::H160;
use serde::{Deserialize, Serialize};

#[derive(Eq, PartialEq, Clone, Debug, Default, Deserialize, Serialize, Hash)]
#[serde(rename_all = "camelCase")]
pub struct Trade {
    pub block_number: u64,
    pub log_index: u64,
    pub order_uid: OrderUid,
    #[serde(with = "serde_with::rust::display_fromstr")]
    pub buy_amount: BigUint,
    #[serde(with = "serde_with::rust::display_fromstr")]
    pub sell_amount: BigUint,
    #[serde(with = "serde_with::rust::display_fromstr")]
    pub sell_amount_before_fees: BigUint,
    // ORDER DATA
    pub owner: H160,
    pub buy_token: H160,
    pub sell_token: H160,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn deserialization_and_back() {
        let value = json!(
        {
            "blockNumber": 1337u64,
            "logIndex": 42u64,
            "orderUid": "0x1111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111",
            "buyAmount": "69",
            "sellAmount": "55",
            "sellAmountBeforeFees": "49",
            "owner": "0x0000000000000000000000000000000000000001",
            "sellToken": "0x000000000000000000000000000000000000000a",
            "buyToken": "0x0000000000000000000000000000000000000009",
        });
        let expected = Trade {
            block_number: 1337u64,
            log_index: 42u64,
            order_uid: OrderUid([17u8; 56]),
            buy_amount: BigUint::from(69u8),
            sell_amount: BigUint::from(55u8),
            sell_amount_before_fees: BigUint::from(49u8),
            owner: H160::from_low_u64_be(1),
            buy_token: H160::from_low_u64_be(9),
            sell_token: H160::from_low_u64_be(10),
        };

        let deserialized: Trade = serde_json::from_value(value.clone()).unwrap();
        assert_eq!(deserialized, expected);
        let serialized = serde_json::to_value(expected).unwrap();
        assert_eq!(serialized, value);
    }
}
