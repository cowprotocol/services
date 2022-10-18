//! Module defining a batch auction.

use crate::{
    order::{OrderData, OrderUid},
    signature::Signature,
    u256_decimal::{self, DecimalU256},
};
use chrono::{DateTime, NaiveDateTime, Utc};
use primitive_types::{H160, U256};
use serde::{Deserialize, Serialize};
use serde_with::serde_as;
use std::collections::BTreeMap;

/// Order information that is relevant to auctions.
///
/// This is different from `crate::orders::Order` because what is relevant to the frontend differs
/// from what is relevant to auction solving.
#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
pub struct Order {
    #[serde(flatten)]
    pub data: OrderData,
    #[serde(flatten)]
    pub metadata: OrderMetadata,
    #[serde(flatten)]
    pub signature: Signature,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OrderMetadata {
    pub uid: OrderUid,
    pub owner: H160,
    pub is_liquidity_order: bool,
    pub creation_date: DateTime<Utc>,
    #[serde(with = "u256_decimal")]
    pub executed_amount: U256,
    #[serde(with = "u256_decimal")]
    pub full_fee_amount: U256,
    /// CIP-14 risk adjusted solver rewards
    ///
    /// Some orders like liquidity orders do not have associated rewards.
    pub reward: f64,
}

impl Default for OrderMetadata {
    fn default() -> Self {
        Self {
            uid: Default::default(),
            owner: Default::default(),
            is_liquidity_order: Default::default(),
            creation_date: DateTime::from_utc(NaiveDateTime::from_timestamp(0, 0), Utc),
            executed_amount: Default::default(),
            full_fee_amount: Default::default(),
            reward: Default::default(),
        }
    }
}

pub type AuctionId = i64;

// Separate type because we usually use the id but store it in the database without as the id is a
// separate column and is autogenerated when we insert the auction.
#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AuctionWithId {
    /// Increments whenever the backend updates the auction.
    ///
    /// Will eventually be synchronized with solution submission for https://github.com/cowprotocol/services/issues/230 .
    pub id: AuctionId,
    #[serde(flatten)]
    pub auction: Auction,
}

/// A batch auction.
#[serde_as]
#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Auction {
    /// The block that this auction is valid for.
    /// The block number for the auction. Orders and prices are guaranteed to be
    /// valid on this block.
    pub block: u64,

    /// The latest block on which a settlement has been processed.
    ///
    /// Note that under certain conditions it is possible for a settlement to
    /// have been mined as part of [`block`] but not have yet been processed.
    pub latest_settlement_block: u64,

    /// The solvable orders included in the auction.
    ///
    /// v1 is included temporarily for backward compatibility.
    #[serde(default, rename = "orders")]
    pub orders_v1: Vec<crate::order::Order>,

    /// The solvable orders included in the auction.
    ///
    /// These are the same orders as v1 but the type definition has changed. Some fields have been
    /// removed and some added.
    #[serde(default, rename = "orders_v2")]
    pub orders: Vec<Order>,

    /// The reference prices for all traded tokens in the auction.
    #[serde_as(as = "BTreeMap<_, DecimalU256>")]
    pub prices: BTreeMap<H160, U256>,
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use super::*;
    use crate::{
        app_id::AppId,
        order::{BuyTokenDestination, OrderKind, SellTokenSource},
        signature::{EcdsaSignature, EcdsaSigningScheme},
    };
    use hex_literal::hex;
    use maplit::btreemap;
    use primitive_types::H256;
    use serde_json::json;

    #[test]
    fn roundtrips_auction() {
        let order = |uid_byte: u8| Order {
            metadata: OrderMetadata {
                uid: OrderUid([uid_byte; 56]),
                ..Default::default()
            },
            ..Default::default()
        };
        let auction = Auction {
            block: 42,
            latest_settlement_block: 40,
            orders_v1: vec![],
            orders: vec![order(1), order(2)],
            prices: btreemap! {
                H160([2; 20]) => U256::from(2),
                H160([1; 20]) => U256::from(1),
            },
        };
        let auction = AuctionWithId { id: 0, auction };

        assert_eq!(
            serde_json::to_value(&auction).unwrap(),
            json!({
                "id": 0,
                "block": 42,
                "latestSettlementBlock": 40,
                "orders": [],
                "orders_v2": [
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
            serde_json::from_value::<AuctionWithId>(serde_json::to_value(&auction).unwrap())
                .unwrap(),
            auction,
        );
    }

    #[test]
    fn roundtrip_order() {
        let value = json!(
        {
            "creationDate": "1970-01-01T00:00:03Z",
            "owner": "0x0000000000000000000000000000000000000001",
            "uid": "0x1111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111",
            "sellToken": "0x000000000000000000000000000000000000000a",
            "buyToken": "0x0000000000000000000000000000000000000009",
            "receiver": "0x000000000000000000000000000000000000000b",
            "sellAmount": "1",
            "buyAmount": "0",
            "executedAmount": "1",
            "validTo": 4294967295u32,
            "appData": "0x6000000000000000000000000000000000000000000000000000000000000007",
            "feeAmount": "115792089237316195423570985008687907853269984665640564039457584007913129639935",
            "fullFeeAmount": "115792089237316195423570985008687907853269984665640564039457584007913129639935",
            "kind": "buy",
            "partiallyFillable": false,
            "signature": "0x0200000000000000000000000000000000000000000000000000000000000003040000000000000000000000000000000000000000000000000000000000000501",
            "signingScheme": "eip712",
            "sellTokenBalance": "external",
            "buyTokenBalance": "internal",
            "isLiquidityOrder": false,
            "reward": 2.,
        });
        let signing_scheme = EcdsaSigningScheme::Eip712;
        let expected = Order {
            metadata: OrderMetadata {
                creation_date: DateTime::<Utc>::from_utc(NaiveDateTime::from_timestamp(3, 0), Utc),
                owner: H160::from_low_u64_be(1),
                uid: OrderUid([17u8; 56]),
                full_fee_amount: U256::MAX,
                is_liquidity_order: false,
                executed_amount: 1.into(),
                reward: 2.,
            },
            data: OrderData {
                sell_token: H160::from_low_u64_be(10),
                buy_token: H160::from_low_u64_be(9),
                receiver: Some(H160::from_low_u64_be(11)),
                sell_amount: 1.into(),
                buy_amount: 0.into(),
                valid_to: u32::MAX,
                app_data: AppId(hex!(
                    "6000000000000000000000000000000000000000000000000000000000000007"
                )),
                fee_amount: U256::MAX,
                kind: OrderKind::Buy,
                partially_fillable: false,
                sell_token_balance: SellTokenSource::External,
                buy_token_balance: BuyTokenDestination::Internal,
            },
            signature: EcdsaSignature {
                v: 1,
                r: H256::from_str(
                    "0200000000000000000000000000000000000000000000000000000000000003",
                )
                .unwrap(),
                s: H256::from_str(
                    "0400000000000000000000000000000000000000000000000000000000000005",
                )
                .unwrap(),
            }
            .to_signature(signing_scheme),
        };
        let deserialized: Order = serde_json::from_value(value.clone()).unwrap();
        assert_eq!(deserialized, expected);
        let serialized = serde_json::to_value(expected).unwrap();
        assert_eq!(serialized, value);
    }
}
