//! Contains the order type as described by the specification with serialization as described by the openapi documentation.

use crate::{h160_hexadecimal, u256_decimal, DomainSeparator, Signature, TokenPair};
use chrono::{offset::Utc, DateTime, NaiveDateTime};
use hex_literal::hex;
use primitive_types::{H160, U256};
use secp256k1::key::ONE_KEY;
use serde::{de, Deserialize, Serialize};
use serde::{Deserializer, Serializer};
use std::fmt::{self, Display};
use std::str::FromStr;
use web3::{
    signing::{self, Key, SecretKeyRef},
    types::Recovery,
};
/// An order that is returned when querying the orderbook.
///
/// Contains extra fields thats are populated by the orderbook.
#[derive(Eq, PartialEq, Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Order {
    #[serde(flatten)]
    pub order_meta_data: OrderMetaData,
    #[serde(flatten)]
    pub order_creation: OrderCreation,
}

impl Default for Order {
    fn default() -> Self {
        Self::from_order_creation(OrderCreation::default(), &DomainSeparator::default()).unwrap()
    }
}

impl Order {
    pub fn from_order_creation(
        order_creation: OrderCreation,
        domain: &DomainSeparator,
    ) -> Option<Self> {
        let owner = order_creation.validate_signature(domain)?;
        Some(Self {
            order_meta_data: OrderMetaData {
                creation_date: chrono::offset::Utc::now(),
                owner,
                uid: order_creation.uid(&owner),
            },
            order_creation,
        })
    }
}

#[derive(Default)]
pub struct OrderBuilder(Order);

impl OrderBuilder {
    pub fn with_sell_token(mut self, sell_token: H160) -> Self {
        self.0.order_creation.sell_token = sell_token;
        self
    }

    pub fn with_buy_token(mut self, buy_token: H160) -> Self {
        self.0.order_creation.buy_token = buy_token;
        self
    }

    pub fn with_sell_amount(mut self, sell_amount: U256) -> Self {
        self.0.order_creation.sell_amount = sell_amount;
        self
    }

    pub fn with_buy_amount(mut self, buy_amount: U256) -> Self {
        self.0.order_creation.buy_amount = buy_amount;
        self
    }

    pub fn with_valid_to(mut self, valid_to: u32) -> Self {
        self.0.order_creation.valid_to = valid_to;
        self
    }

    pub fn with_app_data(mut self, app_data: u32) -> Self {
        self.0.order_creation.app_data = app_data;
        self
    }

    pub fn with_fee_amount(mut self, fee_amount: U256) -> Self {
        self.0.order_creation.fee_amount = fee_amount;
        self
    }

    pub fn with_kind(mut self, kind: OrderKind) -> Self {
        self.0.order_creation.kind = kind;
        self
    }

    pub fn with_partially_fillable(mut self, partially_fillable: bool) -> Self {
        self.0.order_creation.partially_fillable = partially_fillable;
        self
    }

    pub fn with_creation_date(mut self, creation_date: DateTime<Utc>) -> Self {
        self.0.order_meta_data.creation_date = creation_date;
        self
    }

    /// Sets owner, uid, signature.
    pub fn sign_with(mut self, domain_separator: &DomainSeparator, key: SecretKeyRef) -> Self {
        self.0.order_meta_data.owner = key.address();
        self.0.order_meta_data.uid = self.0.order_creation.uid(&key.address());
        self.0.order_creation.sign_self_with(domain_separator, &key);
        self
    }

    pub fn build(self) -> Order {
        self.0
    }
}

/// An order as provided to the orderbook by the frontend.
#[derive(Eq, PartialEq, Clone, Copy, Debug, Deserialize, Serialize)]
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
    pub kind: OrderKind,
    pub partially_fillable: bool,
    pub signature: Signature,
}

impl Default for OrderCreation {
    // Custom implementation to make sure the default order is valid
    fn default() -> Self {
        let mut result = Self {
            sell_token: Default::default(),
            buy_token: Default::default(),
            sell_amount: Default::default(),
            buy_amount: Default::default(),
            valid_to: u32::MAX,
            app_data: Default::default(),
            fee_amount: Default::default(),
            kind: Default::default(),
            partially_fillable: Default::default(),
            signature: Default::default(),
        };
        result.sign_self_with(&DomainSeparator::default(), &SecretKeyRef::new(&ONE_KEY));
        result
    }
}

impl OrderCreation {
    pub fn token_pair(&self) -> Option<TokenPair> {
        TokenPair::new(self.buy_token, self.sell_token)
    }

    // If signature is valid returns the owner.
    pub fn validate_signature(&self, domain_separator: &DomainSeparator) -> Option<H160> {
        // The signature related functionality is defined by the smart contract:
        // https://github.com/gnosis/gp-v2-contracts/blob/main/src/contracts/libraries/GPv2Encoding.sol

        let v = self.signature.v & 0x1f;
        let message = self.signing_digest(domain_separator);
        let recovery = Recovery::new(message, v as u64, self.signature.r, self.signature.s);
        let (signature, recovery_id) = recovery.as_signature()?;
        signing::recover(&message, &signature, recovery_id).ok()
    }

    fn sign_self_with(&mut self, domain_separator: &DomainSeparator, key: &SecretKeyRef) {
        let message = self.signing_digest_message(domain_separator);
        // Unwrap because the only error is for invalid messages which we don't create.
        let signature = Key::sign(key, &message, None).unwrap();
        self.signature.v = signature.v as u8 | 0x80;
        self.signature.r = signature.r;
        self.signature.s = signature.s;
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
impl OrderCreation {}

// See https://github.com/gnosis/gp-v2-contracts/blob/main/src/contracts/libraries/GPv2Encoding.sol
impl OrderCreation {
    pub const ORDER_TYPE_HASH: [u8; 32] =
        hex!("b2b38b9dcbdeb41f7ad71dea9aed79fb47f7bbc3436576fe994b43d5b16ecdec");

    // keccak256("sell")
    const ORDER_KIND_SELL: [u8; 32] =
        hex!("f3b277728b3fee749481eb3e0b3b48980dbbab78658fc419025cb16eee346775");
    // keccak256("buy")
    const ORDER_KIND_BUY: [u8; 32] =
        hex!("6ed88e868af0a1983e3886d5f3e95a2fafbd6c3450bc229e27342283dc429ccc");

    fn order_digest(&self) -> [u8; 32] {
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
        hash_data[256..288].copy_from_slice(match self.kind {
            OrderKind::Sell => &Self::ORDER_KIND_SELL,
            OrderKind::Buy => &Self::ORDER_KIND_BUY,
        });
        hash_data[319] = self.partially_fillable as u8;
        signing::keccak256(&hash_data)
    }

    fn signing_digest_typed_data(&self, domain_separator: &DomainSeparator) -> [u8; 32] {
        let mut hash_data = [0u8; 66];
        hash_data[0..2].copy_from_slice(&[0x19, 0x01]);
        hash_data[2..34].copy_from_slice(&domain_separator.0);
        hash_data[34..66].copy_from_slice(&self.order_digest());
        signing::keccak256(&hash_data)
    }

    fn signing_digest_message(&self, domain_separator: &DomainSeparator) -> [u8; 32] {
        let mut hash_data = [0u8; 92];
        hash_data[0..28].copy_from_slice(b"\x19Ethereum Signed Message:\n64");
        hash_data[28..60].copy_from_slice(&domain_separator.0);
        hash_data[60..92].copy_from_slice(&self.order_digest());
        signing::keccak256(&hash_data)
    }

    fn signing_digest(&self, domain_separator: &DomainSeparator) -> [u8; 32] {
        if self.signature.v & 0x80 == 0 {
            self.signing_digest_typed_data(domain_separator)
        } else {
            self.signing_digest_message(domain_separator)
        }
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
            uid: OrderUid([0u8; 56]),
        }
    }
}

// uid as 56 bytes: 32 for orderDigest, 20 for ownerAddress and 4 for validTo
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct OrderUid(pub [u8; 56]);

impl FromStr for OrderUid {
    type Err = hex::FromHexError;
    fn from_str(s: &str) -> Result<OrderUid, hex::FromHexError> {
        let mut value = [0u8; 56];
        let s_without_prefix = s.strip_prefix("0x").unwrap_or(s);
        hex::decode_to_slice(s_without_prefix, value.as_mut())?;
        Ok(OrderUid(value))
    }
}

impl Display for OrderUid {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut bytes = [0u8; 2 + 56 * 2];
        bytes[..2].copy_from_slice(b"0x");
        // Unwrap because the length is always correct.
        hex::encode_to_slice(&self.0, &mut bytes[2..]).unwrap();
        // Unwrap because the string is always valid utf8.
        let str = std::str::from_utf8(&bytes).unwrap();
        f.write_str(str)
    }
}

impl Default for OrderUid {
    fn default() -> Self {
        Self([0u8; 56])
    }
}

impl Serialize for OrderUid {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(self.to_string().as_str())
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
                let s = s.strip_prefix("0x").ok_or_else(|| {
                    de::Error::custom(format!(
                        "{:?} can't be decoded as hex uid because it does not start with '0x'",
                        s
                    ))
                })?;
                let mut value = [0u8; 56];
                hex::decode_to_slice(s, value.as_mut()).map_err(|err| {
                    de::Error::custom(format!("failed to decode {:?} as hex uid: {}", s, err))
                })?;
                Ok(OrderUid(value))
            }
        }

        deserializer.deserialize_str(Visitor {})
    }
}

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

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::NaiveDateTime;
    use hex_literal::hex;
    use primitive_types::H256;
    use serde_json::json;

    #[test]
    fn deserialization_and_back() {
        let value = json!(
        {
            "creationDate": "1970-01-01T00:00:03Z",
            "owner": "0x0000000000000000000000000000000000000001",
            "uid": "0x1111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111",
            "sellToken": "0x000000000000000000000000000000000000000a",
            "buyToken": "0x0000000000000000000000000000000000000009",
            "sellAmount": "1",
            "buyAmount": "0",
            "validTo": 4294967295u32,
            "appData": 0,
            "feeAmount": "115792089237316195423570985008687907853269984665640564039457584007913129639935",
            "kind": "buy",
            "partiallyFillable": false,
            "signature": "0x0200000000000000000000000000000000000000000000000000000000000003040000000000000000000000000000000000000000000000000000000000000501",
        });
        let expected = Order {
            order_meta_data: OrderMetaData {
                creation_date: DateTime::<Utc>::from_utc(NaiveDateTime::from_timestamp(3, 0), Utc),
                owner: H160::from_low_u64_be(1),
                uid: OrderUid([17u8; 56]),
            },
            order_creation: OrderCreation {
                sell_token: H160::from_low_u64_be(10),
                buy_token: H160::from_low_u64_be(9),
                sell_amount: 1.into(),
                buy_amount: 0.into(),
                valid_to: u32::MAX,
                app_data: 0,
                fee_amount: U256::MAX,
                kind: OrderKind::Buy,
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

    // these two signature tests have been created by printing the order and signature information
    // from the test `should recover signing address for all supported schemes` in
    // https://github.com/gnosis/gp-v2-contracts/blob/main/test/GPv2Encoding.test.ts .
    #[test]
    fn signature_typed_data() {
        let domain_separator = DomainSeparator(hex!(
            "f8a1143d44c67470a791201b239ff6b0ecc8910aa9682bebd08145f5fd84722b"
        ));
        let order = OrderCreation {
            sell_token: hex!("0101010101010101010101010101010101010101").into(),
            buy_token: hex!("0202020202020202020202020202020202020202").into(),
            sell_amount: hex!("0246ddf97976680000").as_ref().into(),
            buy_amount: hex!("b98bc829a6f90000").as_ref().into(),
            valid_to: 4294967295,
            app_data: 0,
            fee_amount: hex!("0de0b6b3a7640000").as_ref().into(),
            kind: OrderKind::Sell,
            partially_fillable: false,
            signature: Signature {
                v: 0x1b,
                r: hex!("41c6a5841abbd04049aa0ec4290487c72d62adcd30054cb4df39988e1ef1a732").into(),
                s: hex!("132ba700917828073a730044f1d8cf280d11faff0c2d43125d910fbf18222c1b").into(),
            },
        };

        let expected_owner = hex!("70997970C51812dc3A010C7d01b50e0d17dc79C8");
        let owner = order.validate_signature(&domain_separator).unwrap();
        assert_eq!(owner, expected_owner.into());
    }

    #[test]
    fn signature_message() {
        let domain_separator = DomainSeparator(hex!(
            "f8a1143d44c67470a791201b239ff6b0ecc8910aa9682bebd08145f5fd84722b"
        ));
        let order = OrderCreation {
            sell_token: hex!("0101010101010101010101010101010101010101").into(),
            buy_token: hex!("0202020202020202020202020202020202020202").into(),
            sell_amount: hex!("0246ddf97976680000").as_ref().into(),
            buy_amount: hex!("b98bc829a6f90000").as_ref().into(),
            valid_to: 4294967295,
            app_data: 0,
            fee_amount: hex!("0de0b6b3a7640000").as_ref().into(),
            kind: OrderKind::Sell,
            partially_fillable: false,
            signature: Signature {
                v: 0x1b | 0x80,
                r: hex!("2c807a9e4f7f72489636d30b33d72b246c6fb467ba203d954ccf2022763a8b21").into(),
                s: hex!("2eb1863e76c37abb54df6695d54f3abc07db4ff7df57e4564a48aee137f358bd").into(),
            },
        };
        let expected_owner = hex!("70997970C51812dc3A010C7d01b50e0d17dc79C8");
        let owner = order.validate_signature(&domain_separator).unwrap();
        assert_eq!(owner, expected_owner.into());
    }

    #[test]
    fn sign_self() {
        let mut order = OrderCreation::default();
        let key = SecretKeyRef::from(&ONE_KEY);
        order.sign_self_with(&DomainSeparator::default(), &key);
        assert_eq!(
            order.validate_signature(&DomainSeparator::default()),
            Some(key.address())
        );
    }

    #[test]
    fn domain_separator_does_not_panic_in_debug() {
        println!("{:?}", DomainSeparator::default());
    }

    #[test]
    fn uid_is_displayed_as_hex() {
        let mut uid = OrderUid([0u8; 56]);
        uid.0[0] = 0x01;
        uid.0[55] = 0xff;
        let expected = "0x01000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000ff";
        assert_eq!(uid.to_string(), expected);
        assert_eq!(format!("{}", uid), expected);
    }
}
