//! Module defining a batch auction.

use crate::{order::Order, u256_decimal::DecimalU256};
use primitive_types::{H160, U256};
use serde::{Deserialize, Serialize};
use serde_with::serde_as;
use std::collections::BTreeMap;

/// A batch auction.
#[serde_as]
#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
pub struct Auction {
    /// The block that this auction is valid for.
    /// The block number for the auction. Orders and prices are guaranteed to be
    /// valid on this block.
    pub block: u64,

    /// The solvable orders included in the auction.
    pub orders: Vec<Order>,

    /// The reference prices for all traded tokens in the auction.
    #[serde_as(as = "BTreeMap<_, DecimalU256>")]
    pub prices: BTreeMap<H160, U256>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::order::{OrderMetaData, OrderUid};
    use maplit::btreemap;
    use serde_json::json;

    #[test]
    fn roundtrips_auction() {
        let order = |uid_byte: u8| Order {
            order_meta_data: OrderMetaData {
                uid: OrderUid([uid_byte; 56]),
                ..Default::default()
            },
            ..Default::default()
        };
        let auction = Auction {
            block: 42,
            orders: vec![order(1), order(2)],
            prices: btreemap! {
                H160([2; 20]) => U256::from(2),
                H160([1; 20]) => U256::from(1),
            },
        };

        assert_eq!(
            serde_json::to_value(&auction).unwrap(),
            json!({
                "block": 42,
                "orders": [
                    order(1),
                    order(2),
                ],
                "prices": {
                    "0x0101010101010101010101010101010101010101": "1",
                    "0x0202020202020202020202020202020202020202": "2",
                },
            }),
        );
        assert_eq!(
            serde_json::from_value::<Auction>(serde_json::to_value(&auction).unwrap()).unwrap(),
            auction,
        );
    }
}
