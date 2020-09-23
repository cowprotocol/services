extern crate anyhow;
extern crate ethabi;
extern crate ethsign;
extern crate rustc_hex;
extern crate serde_json;
extern crate web3;

use anyhow::Result;
use ethabi::encode;
use ethsign::Signature;
use serde::{Deserialize, Serialize};
use std::cmp::Ord;
use std::cmp::Ordering;
use std::cmp::PartialOrd;
use std::ops::Mul;
use web3::contract::tokens::Tokenizable;
use web3::signing::keccak256;
use web3::types::{Address, H160, H256, U256};

#[derive(Debug, Clone, Eq, PartialEq, Deserialize, Serialize, Default)]
pub struct Order {
    pub sell_amount: U256,
    pub buy_amount: U256,
    pub current_sell_amount: U256,
    pub current_buy_amount: U256,
    pub buy_token: Address,
    pub sell_token: Address,
    pub owner: Address,
    pub nonce: u8,
    pub signature_v: u8,
    pub signature_r: H256,
    pub signature_s: H256,
    pub valid_until: U256,
}

impl Order {
    #[allow(dead_code)]
    pub fn get_digest(&self) -> Result<[u8; 32]> {
        let domain_separator: H256 = serde_json::from_str(
            r#""0x24a654ed47680d6a76f087ec92b3a0f0fe4c9c82c26bff3bb22dffe0f120c7f0""#,
        )
        .unwrap();
        return Ok(keccak256(&encode(&[
            domain_separator.into_token(),
            self.sell_amount.into_token(),
            self.buy_amount.into_token(),
            self.sell_token.into_token(),
            self.buy_token.into_token(),
            self.owner.into_token(),
            self.nonce.into_token(),
        ])));
    }
    #[allow(dead_code)]
    pub fn validate_order(&self) -> Result<bool> {
        let message = self.get_digest()?;
        let signature = Signature {
            v: self.signature_v - 27 as u8,
            r: *self.signature_r.as_fixed_bytes(),
            s: *self.signature_s.as_fixed_bytes(),
        };
        let owner = signature.recover(&message)?;
        return Ok(H160::from(owner.address()).eq(&self.owner));
    }
    #[cfg(test)]
    pub fn new_valid_test_order() -> Self {
        Order {
            sell_amount: U256::from_dec_str("1000000000000000000").unwrap(),
            buy_amount: U256::from_dec_str("900000000000000000").unwrap(),
            current_sell_amount: U256::from_dec_str("1000000000000000000").unwrap(),
            current_buy_amount: U256::from_dec_str("900000000000000000").unwrap(),
            sell_token: serde_json::from_str(r#""0xA193E42526F1FEA8C99AF609dcEabf30C1c29fAA""#)
                .unwrap(),
            buy_token: serde_json::from_str(r#""0xFDFEF9D10d929cB3905C71400ce6be1990EA0F34""#)
                .unwrap(),
            owner: serde_json::from_str(r#""0x63FC2aD3d021a4D7e64323529a55a9442C444dA0""#).unwrap(),
            nonce: 1,
            signature_v: 27 as u8,
            signature_r: serde_json::from_str(
                r#""0x07cf23fa6f588cc3a91de8444b589e5afbf91c5d486c512a353d45d02fa58700""#,
            )
            .unwrap(),
            signature_s: serde_json::from_str(
                r#""0x53671e75b62b5bd64f91c80430aafb002040c35d1fcf25d0dc55d978946d5c11""#,
            )
            .unwrap(),
            valid_until: U256::from("0"),
        }
    }
}

impl Ord for Order {
    fn cmp(&self, other: &Self) -> Ordering {
        (self.sell_amount.mul(other.buy_amount)).cmp(&self.buy_amount.mul(other.sell_amount))
    }
}

impl PartialOrd for Order {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        (self.sell_amount.mul(other.buy_amount))
            .partial_cmp(&self.buy_amount.mul(other.sell_amount))
    }
}

#[cfg(test)]
pub mod test_util {
    use super::*;
    use rustc_hex::FromHex;

    #[test]
    fn test_validates_valid_order() {
        let order = Order::new_valid_test_order();
        let result = order.validate_order().unwrap();
        assert_eq!(result, true);
    }

    #[test]
    fn test_invalidates_invalid_order() {
        let mut order = Order::new_valid_test_order();
        order.signature_v = 28;
        let result = order.validate_order().unwrap();
        assert_eq!(result, false);
    }

    #[test]
    fn test_order_of_orders() {
        let mut order_1 = Order::new_valid_test_order();
        let order_2 = Order::new_valid_test_order();
        order_1.sell_amount = order_1.sell_amount.checked_add(U256::one()).unwrap();
        assert_eq!(order_1.cmp(&order_2), Ordering::Greater);
    }

    #[test]
    fn test_get_digest() {
        let order = Order::new_valid_test_order();

        let result = order.get_digest().unwrap();
        let expected_result = "0e9aab5c9680276d90a87387b533197feb6ac7812fb80fa49de40fcd9bee8166";
        let expected_bytes: Vec<u8> = expected_result.from_hex().unwrap();

        assert_eq!(result.to_vec(), expected_bytes);
    }
}
