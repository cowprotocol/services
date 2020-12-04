//! Contains the order type as described by the specification with serialization as described by the openapi documentation.
//!
//! This is in its own crate because we want to share this module between the orderbook and the solver.

pub mod h160_hexadecimal;
pub mod u256_decimal;

use chrono::{offset::Utc, DateTime, NaiveDateTime};
use primitive_types::{H160, H256, U256};
use serde::{de, Deserialize, Serialize};
use serde::{Deserializer, Serializer};
use std::fmt;

#[derive(Eq, PartialEq, Clone, Copy, Debug, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum OrderKind {
    Buy,
    Sell,
}

impl Default for OrderKind {
    fn default() -> Self {
        Self::Buy
    }
}

#[derive(Eq, PartialEq, Clone, Copy, Debug, Default)]
pub struct Signature {
    pub v: u8,
    pub r: H256,
    pub s: H256,
}

/// An order as provided to the orderbook by the frontend.
#[derive(Eq, PartialEq, Clone, Copy, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OrderCreation {
    #[serde(with = "h160_hexadecimal")]
    pub sell_token: H160,
    #[serde(with = "h160_hexadecimal")]
    pub buy_token: H160,
    #[serde(with = "u256_decimal")]
    pub sell_amount: U256,
    #[serde(with = "u256_decimal")]
    pub buy_amount: U256,
    pub valid_to: u32,
    pub app_data: u32,
    #[serde(with = "u256_decimal")]
    pub fee_amount: U256,
    pub order_kind: OrderKind,
    pub partially_fillable: bool,
    pub signature: Signature,
}

impl OrderCreation {
    pub fn token_pair(&self) -> Option<TokenPair> {
        TokenPair::new(self.buy_token, self.sell_token)
    }
}

// uid as 56 bytes: 32 for orderDigest, 20 for ownerAddress and 4 for validTo
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct OrderUid(pub [u8; 56]);

impl Serialize for OrderUid {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&hex::encode(self.0.iter()))
    }
}

impl<'de> Deserialize<'de> for OrderUid {
    fn deserialize<D>(deserializer: D) -> Result<OrderUid, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct Visitor {}
        impl<'de> de::Visitor<'de> for Visitor {
            type Value = OrderUid;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                write!(formatter, "an uid with orderDigest_owner_validTo")
            }

            fn visit_str<E>(self, s: &str) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                let mut value = [0 as u8; 56];
                hex::decode_to_slice(s, value.as_mut()).map_err(|err| {
                    de::Error::custom(format!("failed to decode {:?} as hex: {}", s, err))
                })?;
                Ok(OrderUid(value))
            }
        }

        deserializer.deserialize_str(Visitor {})
    }
}

/// An order as provided to the orderbook by the frontend.
#[derive(Eq, PartialEq, Clone, Debug, Copy, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OrderMetaData {
    pub creation_date: DateTime<Utc>,
    #[serde(with = "h160_hexadecimal")]
    pub owner: H160,
    pub uid: OrderUid,
}

impl Default for OrderMetaData {
    fn default() -> Self {
        Self {
            creation_date: DateTime::from_utc(NaiveDateTime::from_timestamp(0, 0), Utc),
            owner: Default::default(),
            uid: OrderUid([0 as u8; 56]),
        }
    }
}

/// An order that is returned when querying the orderbook.
///
/// Contains extra fields thats are populated by the orderbook.
#[derive(Eq, PartialEq, Clone, Default, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Order {
    #[serde(flatten)]
    pub order_meta_data: OrderMetaData,
    #[serde(flatten)]
    pub order_creation: OrderCreation,
}

impl Serialize for Signature {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let mut bytes = [0u8; 65 * 2];
        // Can only fail if the buffer size does not match but we know it is correct.
        hex::encode_to_slice([self.v], &mut bytes[..2]).unwrap();
        hex::encode_to_slice(self.r, &mut bytes[2..66]).unwrap();
        hex::encode_to_slice(self.s, &mut bytes[66..]).unwrap();
        // Hex encoding is always valid utf8.
        let str = std::str::from_utf8(&bytes).unwrap();
        serializer.serialize_str(str)
    }
}

impl<'de> Deserialize<'de> for Signature {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        struct Visitor {}
        impl<'de> de::Visitor<'de> for Visitor {
            type Value = Signature;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                write!(formatter, "the 65 signature bytes as a hex encoded string")
            }

            fn visit_str<E>(self, s: &str) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                let mut bytes = [0u8; 65];
                hex::decode_to_slice(s, &mut bytes).map_err(|err| {
                    de::Error::custom(format!("failed to decode {:?} as hex: {}", s, err))
                })?;
                Ok(Signature {
                    v: bytes[0],
                    r: H256::from_slice(&bytes[1..33]),
                    s: H256::from_slice(&bytes[33..]),
                })
            }
        }

        deserializer.deserialize_str(Visitor {})
    }
}

/// Erc20 token pair specified by two contract addresses.
#[derive(Eq, PartialEq, Copy, Clone, Debug, Hash, Ord, PartialOrd)]
pub struct TokenPair(H160, H160);

impl TokenPair {
    /// Create a new token pair from two addresses.
    /// The addresses must not be the equal.
    pub fn new(token_a: H160, token_b: H160) -> Option<Self> {
        match token_a.cmp(&token_b) {
            std::cmp::Ordering::Less => Some(Self(token_a, token_b)),
            std::cmp::Ordering::Equal => None,
            std::cmp::Ordering::Greater => Some(Self(token_b, token_a)),
        }
    }

    /// The first address is always the lower one.
    /// The addresses are never equal.
    pub fn get(&self) -> (H160, H160) {
        (self.0, self.1)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::NaiveDateTime;
    use serde_json::json;
    use std::str::FromStr;

    #[test]
    fn deserialization_and_back() {
        let value = json!(
        {
            "creationDate": "1970-01-01T00:00:03Z",
            "owner": "0000000000000000000000000000000000000001",
            "uid": "1111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111",
            "sellToken": "000000000000000000000000000000000000000a",
            "buyToken": "0000000000000000000000000000000000000009",
            "sellAmount": "1",
            "buyAmount": "0",
            "validTo": 4294967295u32,
            "appData": 0,
            "feeAmount": "115792089237316195423570985008687907853269984665640564039457584007913129639935",
            "orderKind": "buy",
            "partiallyFillable": false,
            "signature": "0102000000000000000000000000000000000000000000000000000000000000030400000000000000000000000000000000000000000000000000000000000005",
        });
        let expected = Order {
            order_meta_data: OrderMetaData {
                creation_date: DateTime::<Utc>::from_utc(NaiveDateTime::from_timestamp(3, 0), Utc),
                owner: H160::from_low_u64_be(1),
                uid: OrderUid([17 as u8; 56]),
            },
            order_creation: OrderCreation {
                sell_token: H160::from_low_u64_be(10),
                buy_token: H160::from_low_u64_be(9),
                sell_amount: 1.into(),
                buy_amount: 0.into(),
                valid_to: u32::MAX,
                app_data: 0,
                fee_amount: U256::MAX,
                order_kind: OrderKind::Buy,
                partially_fillable: false,
                signature: Signature {
                    v: 1,
                    r: H256::from_str(
                        "0200000000000000000000000000000000000000000000000000000000000003",
                    )
                    .unwrap(),
                    s: H256::from_str(
                        "0400000000000000000000000000000000000000000000000000000000000005",
                    )
                    .unwrap(),
                },
            },
        };
        let deserialized: Order = serde_json::from_value(value.clone()).unwrap();
        assert_eq!(deserialized, expected);
        let serialized = serde_json::to_value(expected).unwrap();
        assert_eq!(serialized, value);
    }

    #[test]
    fn token_pair_is_sorted() {
        let token_a = H160::from_low_u64_be(0);
        let token_b = H160::from_low_u64_be(1);
        let pair_0 = TokenPair::new(token_a, token_b).unwrap();
        let pair_1 = TokenPair::new(token_b, token_a).unwrap();
        assert_eq!(pair_0, pair_1);
        assert_eq!(pair_0.get(), pair_1.get());
        assert_eq!(pair_0.get().0, token_a);
    }

    #[test]
    fn token_pair_cannot_be_equal() {
        let token = H160::from_low_u64_be(1);
        assert_eq!(TokenPair::new(token, token), None);
    }
}
