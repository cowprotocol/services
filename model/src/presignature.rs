//! Contains the Presignature type as described by the specification with serialization as described by the openapi documentation.

use crate::order::OrderUid;
use primitive_types::H160;
use serde::{Deserialize, Serialize};

#[derive(Eq, PartialEq, Clone, Debug, Default, Deserialize, Serialize, Hash)]
#[serde(rename_all = "camelCase")]
pub struct PreSignature {
    pub block_number: u64,
    pub log_index: u64,
    pub owner: H160,
    pub order_uid: OrderUid,
    pub signed: bool,
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
            "owner": "0x0000000000000000000000000000000000000042",
            "orderUid": "0x1111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111",
            "signed": true,
        });
        let expected = PreSignature {
            block_number: 1337u64,
            log_index: 42u64,
            owner: H160::from_low_u64_be(0x42),
            order_uid: OrderUid([0x11u8; 56]),
            signed: true,
        };

        let deserialized: PreSignature = serde_json::from_value(value.clone()).unwrap();
        assert_eq!(deserialized, expected);
        let serialized = serde_json::to_value(expected).unwrap();
        assert_eq!(serialized, value);
    }
}
