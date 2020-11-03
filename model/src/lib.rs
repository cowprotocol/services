//! Contains the order type as described by the specification with serialization as described by the openapi documentation.
//!
//! This is in its own crate because we want to share this module between the orderbook and the solver.

use chrono::{offset::Utc, DateTime, NaiveDateTime};
use primitive_types::{H160, H256};
use serde::{de, Deserialize, Serialize};
use std::fmt;

#[derive(Eq, PartialEq, Clone, Copy, Debug, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum FillType {
    FillOrKill,
    Partial,
}

impl Default for FillType {
    fn default() -> Self {
        Self::FillOrKill
    }
}

#[derive(Eq, PartialEq, Clone, Copy, Debug, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum OrderType {
    Buy,
    Sell,
}

impl Default for OrderType {
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
pub struct UserOrder {
    #[serde(with = "h160_hex")]
    pub buy_token: H160,
    #[serde(with = "serde_with::rust::display_fromstr")]
    pub buy_amount: u128,
    #[serde(with = "h160_hex")]
    pub sell_token: H160,
    #[serde(with = "serde_with::rust::display_fromstr")]
    pub sell_amount: u128,
    #[serde(with = "serde_with::rust::display_fromstr")]
    pub sell_token_tip: u128,
    pub order_type: OrderType,
    pub fill_type: FillType,
    pub nonce: u32,
    pub valid_to: u32,
    pub signature: Signature,
}

impl UserOrder {
    pub fn token_pair(&self) -> Option<TokenPair> {
        TokenPair::new(self.buy_token, self.sell_token)
    }
}

/// An order that is returned when querying the orderbook.
///
/// Contains extra fields thats are populated by the orderbook.
#[derive(Eq, PartialEq, Clone, Copy, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Order {
    pub creation_time: DateTime<Utc>,
    #[serde(with = "h160_hex")]
    pub owner: H160,
    #[serde(flatten)]
    pub user_provided: UserOrder,
}

impl Default for Order {
    fn default() -> Self {
        Self {
            creation_time: DateTime::from_utc(NaiveDateTime::from_timestamp(0, 0), Utc),
            owner: Default::default(),
            user_provided: Default::default(),
        }
    }
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

mod h160_hex {
    use primitive_types::H160;
    use serde::{de, Deserializer, Serializer};
    use std::fmt;

    pub fn serialize<S>(value: &H160, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut bytes = [0u8; 20 * 2];
        // Can only fail if the buffer size does not match but we know it is correct.
        hex::encode_to_slice(value, &mut bytes).unwrap();
        // Hex encoding is always valid utf8.
        let s = std::str::from_utf8(&bytes).unwrap();
        serializer.serialize_str(s)
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<H160, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct Visitor {}
        impl<'de> de::Visitor<'de> for Visitor {
            type Value = H160;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                write!(formatter, "an ethereum address as a hex encoded string")
            }

            fn visit_str<E>(self, s: &str) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                let mut value = H160::zero();
                hex::decode_to_slice(s, value.as_mut()).map_err(|err| {
                    de::Error::custom(format!("failed to decode {:?} as hex: {}", s, err))
                })?;
                Ok(value)
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
          "owner": "0000000000000000000000000000000000000001",
          "creationTime": "1970-01-01T00:00:03Z",
          "buyToken": "0000000000000000000000000000000000000009",
          "buyAmount": "0",
          "sellToken": "000000000000000000000000000000000000000a",
          "sellAmount": "1",
          "sellTokenTip": "5192296858534827628530496329220095",
          "orderType": "buy",
          "fillType": "fillorkill",
          "nonce": 0,
          "validTo": 4294967295u32,
          "signature": "0102000000000000000000000000000000000000000000000000000000000000030400000000000000000000000000000000000000000000000000000000000005",
        });
        let expected = Order {
            creation_time: DateTime::<Utc>::from_utc(NaiveDateTime::from_timestamp(3, 0), Utc),
            owner: H160::from_low_u64_be(1),
            user_provided: UserOrder {
                buy_token: H160::from_low_u64_be(9),
                buy_amount: 0,
                sell_token: H160::from_low_u64_be(10),
                sell_amount: 1,
                sell_token_tip: 2u128.pow(112) - 1,
                order_type: OrderType::Buy,
                fill_type: FillType::FillOrKill,
                nonce: 0,
                valid_to: u32::MAX,
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
