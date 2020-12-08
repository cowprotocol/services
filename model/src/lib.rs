//! Contains the order type as described by the specification with serialization as described by the openapi documentation.
//!
//! This is in its own crate because we want to share this module between the orderbook and the solver.

pub mod h160_hexadecimal;
pub mod u256_decimal;
use chrono::{offset::Utc, DateTime, NaiveDateTime};
use hex_literal::hex;
use primitive_types::{H160, H256, U256};
use secp256k1::{constants::SECRET_KEY_SIZE, SecretKey};
use serde::{de, Deserialize, Serialize};
use serde::{Deserializer, Serializer};
use std::fmt;
use web3::{
    signing::{self, Key, SecretKeyRef},
    types::Recovery,
};

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
    pub const ORDER_TYPE_HASH: [u8; 32] =
        hex!("b71968fcf5e55b9c3370f2809d4078a4695be79dfa43e5aa1f2baa0a9b84f186");

    pub fn token_pair(&self) -> Option<TokenPair> {
        TokenPair::new(self.buy_token, self.sell_token)
    }

    pub fn order_digest(&self) -> [u8; 32] {
        let mut hash_data = [0u8; 320];
        hash_data[0..32].copy_from_slice(&Self::ORDER_TYPE_HASH);
        // Some slots are not assigned (stay 0) because all values are extended to 256 bits.
        hash_data[44..64].copy_from_slice(self.sell_token.as_fixed_bytes());
        hash_data[76..96].copy_from_slice(self.buy_token.as_fixed_bytes());
        self.sell_amount.to_big_endian(&mut hash_data[96..128]);
        self.buy_amount.to_big_endian(&mut hash_data[128..160]);
        hash_data[188..192].copy_from_slice(&self.valid_to.to_be_bytes());
        hash_data[220..224].copy_from_slice(&self.app_data.to_be_bytes());
        self.fee_amount.to_big_endian(&mut hash_data[224..256]);
        hash_data[287] = match self.order_kind {
            OrderKind::Sell => 0,
            OrderKind::Buy => 1,
        };
        hash_data[319] = self.partially_fillable as u8;
        signing::keccak256(&hash_data)
    }

    pub fn signing_digest_typed_data(&self, domain_separator: &[u8; 32]) -> [u8; 32] {
        let mut hash_data = [0u8; 66];
        hash_data[0..2].copy_from_slice(&[0x19, 0x01]);
        hash_data[2..34].copy_from_slice(domain_separator);
        hash_data[34..66].copy_from_slice(&self.order_digest());
        signing::keccak256(&hash_data)
    }

    pub fn signing_digest_message(&self, domain_separator: &[u8; 32]) -> [u8; 32] {
        let mut hash_data = [0u8; 92];
        hash_data[0..28].copy_from_slice(b"\x19Ethereum Signed Message:\n64");
        hash_data[28..60].copy_from_slice(domain_separator);
        hash_data[60..92].copy_from_slice(&self.order_digest());
        signing::keccak256(&hash_data)
    }

    pub fn signing_digest(&self, domain_separator: &[u8; 32]) -> [u8; 32] {
        if self.signature.v & 0x80 == 0 {
            self.signing_digest_typed_data(domain_separator)
        } else {
            self.signing_digest_message(domain_separator)
        }
    }

    // If signature is valid returns the owner.
    pub fn validate_signature(&self, domain_separator: &[u8; 32]) -> Option<H160> {
        let v = self.signature.v & 0x1f;
        let message = self.signing_digest(domain_separator);
        let recovery = Recovery::new(message, v as u64, self.signature.r, self.signature.s);
        let (signature, recovery_id) = recovery.as_signature()?;
        signing::recover(&message, &signature, recovery_id).ok()
    }

    pub fn uid(&self, owner: &H160) -> OrderUid {
        let mut uid = OrderUid([0u8; 56]);
        uid.0[0..32].copy_from_slice(&self.order_digest());
        uid.0[32..52].copy_from_slice(owner.as_fixed_bytes());
        uid.0[52..56].copy_from_slice(&self.valid_to.to_be_bytes());
        uid
    }
}

// Intended to be used by tests that need signed orders.
impl OrderCreation {
    pub const TEST_DOMAIN_SEPARATOR: [u8; 32] = [0u8; 32];

    pub fn sign_self_with(&mut self, domain_separator: &[u8; 32], key: SecretKeyRef) {
        let message = self.signing_digest_message(domain_separator);
        // Unwrap because the only error is for invalid messages which we don't create.
        let signature = Key::sign(&key, &message, None).unwrap();
        self.signature.v = signature.v as u8 | 0x80;
        self.signature.r = signature.r;
        self.signature.s = signature.s;
    }

    // Picks the test domain and an arbitrary secret key. Returns the corresponding address.
    pub fn sign_self(&mut self) -> H160 {
        let key = SecretKey::from_slice(&[1u8; SECRET_KEY_SIZE]).unwrap();
        let key_ref = SecretKeyRef::new(&key);
        let address = web3::signing::Key::address(&key_ref);
        self.sign_self_with(&Self::TEST_DOMAIN_SEPARATOR, key_ref);
        address
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

    #[test]
    fn signature_typed_data() {
        // copied from a gp-v2-contracts test
        let domain_separator =
            hex!("f8a1143d44c67470a791201b239ff6b0ecc8910aa9682bebd08145f5fd84722b");
        let order = OrderCreation {
            sell_token: hex!("0101010101010101010101010101010101010101").into(),
            buy_token: hex!("0202020202020202020202020202020202020202").into(),
            sell_amount: hex!("0303030303030303030303030303030303030303030303030303030303030303")
                .into(),
            buy_amount: hex!("0404040404040404040404040404040404040404040404040404040404040404")
                .into(),
            valid_to: 84215045,
            app_data: 101058054,
            fee_amount: hex!("0707070707070707070707070707070707070707070707070707070707070707")
                .into(),
            order_kind: OrderKind::Buy,
            partially_fillable: true,
            signature: Signature {
                v: 0x1b,
                r: hex!("159bad4cdce8e9eeb34f4450941e6e512cd2ceaba2809f6928ad91ce58700064").into(),
                s: hex!("4cd8e52110ca7d1ba7493828bb811969115aff9d8358a5071bd2e7d15c1362bd").into(),
            },
        };
        let expected_owner = hex!("70997970C51812dc3A010C7d01b50e0d17dc79C8");
        let owner = order.validate_signature(&domain_separator).unwrap();
        assert_eq!(owner, expected_owner.into());
    }

    #[test]
    fn signature_message() {
        // copied from a gp-v2-contracts test
        let domain_separator =
            hex!("f8a1143d44c67470a791201b239ff6b0ecc8910aa9682bebd08145f5fd84722b");
        let order = OrderCreation {
            sell_token: hex!("0101010101010101010101010101010101010101").into(),
            buy_token: hex!("0202020202020202020202020202020202020202").into(),
            sell_amount: hex!("0246ddf97976680000").as_ref().into(),
            buy_amount: hex!("b98bc829a6f90000").as_ref().into(),
            valid_to: 4294967295,
            app_data: 0,
            fee_amount: hex!("0de0b6b3a7640000").as_ref().into(),
            order_kind: OrderKind::Sell,
            partially_fillable: false,
            signature: Signature {
                v: 0x1b | 0x80,
                r: hex!("97be7a77d916c99b39d2909c5fde76a82f4fd49868b18d000d93de2c59061602").into(),
                s: hex!("68f04221fd6ac0e8aebfb048cf6c75ca882cd20c6e06aec094c278f8e44c1759").into(),
            },
        };
        let expected_owner = hex!("70997970C51812dc3A010C7d01b50e0d17dc79C8");
        let owner = order.validate_signature(&domain_separator).unwrap();
        assert_eq!(owner, expected_owner.into());
    }

    #[test]
    fn sign_self() {
        let mut order = OrderCreation::default();
        let owner = order.sign_self();
        assert_eq!(
            order.validate_signature(&OrderCreation::TEST_DOMAIN_SEPARATOR),
            Some(owner)
        );
    }
}
