//! Contains the order type as described by the specification with serialization as described by the openapi documentation.

use crate::{
    appdata_hexadecimal,
    h160_hexadecimal::{self, HexadecimalH160},
    u256_decimal::{self, DecimalU256},
    DomainSeparator, Signature, SigningScheme, TokenPair,
};
use chrono::{offset::Utc, DateTime, NaiveDateTime};
use hex_literal::hex;
use num_bigint::BigUint;
use primitive_types::{H160, U256};
use secp256k1::key::ONE_KEY;
use serde::{de, Deserialize, Serialize};
use serde::{Deserializer, Serializer};
use serde_with::serde_as;
use std::collections::HashSet;
use std::fmt::{self, Display};
use std::str::FromStr;
use web3::signing::{self, Key, SecretKeyRef};

/// The flag denoting that an order is buying ETH (or the chain's native token).
/// It is used in place of an actual buy token address in an order.
pub const BUY_ETH_ADDRESS: H160 = H160([0xee; 20]);

/// An order that is returned when querying the orderbook.
///
/// Contains extra fields that are populated by the orderbook.
#[derive(Eq, PartialEq, Clone, Debug, Deserialize, Serialize, Hash)]
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
        let owner = order_creation.signature.validate(
            order_creation.signing_scheme,
            domain,
            &order_creation.hash_struct(),
        )?;
        Some(Self {
            order_meta_data: OrderMetaData {
                creation_date: chrono::offset::Utc::now(),
                owner,
                uid: order_creation.uid(domain, &owner),
                ..Default::default()
            },
            order_creation,
        })
    }
    pub fn contains_token_from(&self, token_list: &HashSet<H160>) -> bool {
        token_list.contains(&self.order_creation.buy_token)
            || token_list.contains(&self.order_creation.sell_token)
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

    pub fn with_app_data(mut self, app_data: [u8; 32]) -> Self {
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

    pub fn with_signing_scheme(mut self, signing_scheme: SigningScheme) -> Self {
        self.0.order_creation.signing_scheme = signing_scheme;
        self
    }

    /// Sets owner, uid, signature.
    pub fn sign_with(mut self, domain: &DomainSeparator, key: SecretKeyRef) -> Self {
        self.0.order_meta_data.owner = key.address();
        self.0.order_meta_data.uid = self.0.order_creation.uid(domain, &key.address());
        self.0.order_creation.signature = Signature::sign(
            self.0.order_creation.signing_scheme,
            domain,
            &self.0.order_creation.hash_struct(),
            key,
        );
        self
    }

    pub fn build(self) -> Order {
        self.0
    }
}

/// An order as provided to the orderbook by the frontend.
#[serde_as]
#[derive(Eq, PartialEq, Clone, Copy, Debug, Deserialize, Serialize, Hash)]
#[serde(rename_all = "camelCase")]
pub struct OrderCreation {
    #[serde(with = "h160_hexadecimal")]
    pub sell_token: H160,
    #[serde(with = "h160_hexadecimal")]
    pub buy_token: H160,
    #[serde(default)]
    #[serde_as(as = "Option<HexadecimalH160>")]
    pub receiver: Option<H160>,
    #[serde(with = "u256_decimal")]
    pub sell_amount: U256,
    #[serde(with = "u256_decimal")]
    pub buy_amount: U256,
    pub valid_to: u32,
    #[serde(with = "appdata_hexadecimal")]
    pub app_data: [u8; 32],
    #[serde(with = "u256_decimal")]
    pub fee_amount: U256,
    pub kind: OrderKind,
    pub partially_fillable: bool,
    pub signature: Signature,
    pub signing_scheme: SigningScheme,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct OrderCreationPayload {
    #[serde(flatten)]
    pub order_creation: OrderCreation,
    pub from: Option<H160>,
}

impl Default for OrderCreation {
    // Custom implementation to make sure the default order is valid
    fn default() -> Self {
        let mut result = Self {
            sell_token: Default::default(),
            buy_token: Default::default(),
            receiver: Default::default(),
            sell_amount: Default::default(),
            buy_amount: Default::default(),
            valid_to: u32::MAX,
            app_data: Default::default(),
            fee_amount: Default::default(),
            kind: Default::default(),
            partially_fillable: Default::default(),
            signing_scheme: SigningScheme::Eip712,
            signature: Default::default(),
        };
        result.signature = Signature::sign(
            result.signing_scheme,
            &DomainSeparator::default(),
            &result.hash_struct(),
            SecretKeyRef::new(&ONE_KEY),
        );
        result
    }
}

impl OrderCreation {
    pub fn token_pair(&self) -> Option<TokenPair> {
        TokenPair::new(self.buy_token, self.sell_token)
    }

    pub fn uid(&self, domain: &DomainSeparator, owner: &H160) -> OrderUid {
        let mut uid = OrderUid([0u8; 56]);
        uid.0[0..32].copy_from_slice(&super::hashed_eip712_message(domain, &self.hash_struct()));
        uid.0[32..52].copy_from_slice(owner.as_fixed_bytes());
        uid.0[52..56].copy_from_slice(&self.valid_to.to_be_bytes());
        uid
    }
}

// EIP-712
impl OrderCreation {
    // See https://github.com/gnosis/gp-v2-contracts/blob/main/src/contracts/libraries/GPv2Encoding.sol
    pub const ORDER_TYPE_HASH: [u8; 32] =
        hex!("d604be04a8c6d2df582ec82eba9b65ce714008acbf9122dd95e499569c8f1a80");
    // keccak256("sell")
    const ORDER_KIND_SELL: [u8; 32] =
        hex!("f3b277728b3fee749481eb3e0b3b48980dbbab78658fc419025cb16eee346775");
    // keccak256("buy")
    const ORDER_KIND_BUY: [u8; 32] =
        hex!("6ed88e868af0a1983e3886d5f3e95a2fafbd6c3450bc229e27342283dc429ccc");

    pub fn hash_struct(&self) -> [u8; 32] {
        let mut hash_data = [0u8; 352];
        hash_data[0..32].copy_from_slice(&Self::ORDER_TYPE_HASH);
        // Some slots are not assigned (stay 0) because all values are extended to 256 bits.
        hash_data[44..64].copy_from_slice(self.sell_token.as_fixed_bytes());
        hash_data[76..96].copy_from_slice(self.buy_token.as_fixed_bytes());
        hash_data[108..128]
            .copy_from_slice(self.receiver.unwrap_or_else(H160::zero).as_fixed_bytes());
        self.sell_amount.to_big_endian(&mut hash_data[128..160]);
        self.buy_amount.to_big_endian(&mut hash_data[160..192]);
        hash_data[220..224].copy_from_slice(&self.valid_to.to_be_bytes());
        hash_data[224..256].copy_from_slice(&self.app_data);
        self.fee_amount.to_big_endian(&mut hash_data[256..288]);
        hash_data[288..320].copy_from_slice(match self.kind {
            OrderKind::Sell => &Self::ORDER_KIND_SELL,
            OrderKind::Buy => &Self::ORDER_KIND_BUY,
        });
        hash_data[351] = self.partially_fillable as u8;
        signing::keccak256(&hash_data)
    }
}

/// An order cancellation as provided to the orderbook by the frontend.
#[serde_as]
#[derive(Eq, PartialEq, Clone, Copy, Debug, Hash)]
pub struct OrderCancellation {
    pub order_uid: OrderUid,
    pub signature: Signature,
    pub signing_scheme: SigningScheme,
}

impl Default for OrderCancellation {
    fn default() -> Self {
        let mut result = Self {
            order_uid: OrderUid::default(),
            signature: Default::default(),
            signing_scheme: SigningScheme::Eip712,
        };
        result.signature = Signature::sign(
            result.signing_scheme,
            &DomainSeparator::default(),
            &result.hash_struct(),
            SecretKeyRef::new(&ONE_KEY),
        );
        result
    }
}

// EIP-712
impl OrderCancellation {
    // keccak256("OrderCancellation(bytes orderUid)")
    const ORDER_CANCELLATION_TYPE_HASH: [u8; 32] =
        hex!("7b41b3a6e2b3cae020a3b2f9cdc997e0d420643957e7fea81747e984e47c88ec");

    pub fn hash_struct(&self) -> [u8; 32] {
        let mut hash_data = [0u8; 64];
        hash_data[0..32].copy_from_slice(&Self::ORDER_CANCELLATION_TYPE_HASH);
        hash_data[32..64].copy_from_slice(&signing::keccak256(&self.order_uid.0));
        signing::keccak256(&hash_data)
    }

    pub fn validate(&self, domain_separator: &DomainSeparator) -> Option<H160> {
        self.signature
            .validate(self.signing_scheme, domain_separator, &self.hash_struct())
    }
}

/// An order as provided to the orderbook by the frontend.
#[serde_as]
#[derive(Eq, PartialEq, Clone, Debug, Deserialize, Serialize, Hash)]
#[serde(rename_all = "camelCase")]
pub struct OrderMetaData {
    pub creation_date: DateTime<Utc>,
    #[serde(with = "h160_hexadecimal")]
    pub owner: H160,
    pub uid: OrderUid,
    #[serde_as(as = "Option<DecimalU256>")]
    pub available_balance: Option<U256>,
    #[serde(with = "serde_with::rust::display_fromstr")]
    pub executed_buy_amount: BigUint,
    #[serde(with = "serde_with::rust::display_fromstr")]
    pub executed_sell_amount: BigUint,
    #[serde(with = "serde_with::rust::display_fromstr")]
    pub executed_sell_amount_before_fees: BigUint,
    #[serde(with = "serde_with::rust::display_fromstr")]
    pub executed_fee_amount: BigUint,
    pub invalidated: bool,
}

impl Default for OrderMetaData {
    fn default() -> Self {
        Self {
            creation_date: DateTime::from_utc(NaiveDateTime::from_timestamp(0, 0), Utc),
            owner: Default::default(),
            uid: Default::default(),
            available_balance: Default::default(),
            executed_buy_amount: Default::default(),
            executed_sell_amount: Default::default(),
            executed_sell_amount_before_fees: Default::default(),
            executed_fee_amount: Default::default(),
            invalidated: Default::default(),
        }
    }
}

// uid as 56 bytes: 32 for orderDigest, 20 for ownerAddress and 4 for validTo
#[derive(Clone, Copy, Eq, Hash, PartialEq)]
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

impl fmt::Debug for OrderUid {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self)
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

#[derive(Eq, PartialEq, Clone, Copy, Debug, Deserialize, Serialize, Hash, enum_utils::FromStr)]
#[enumeration(case_insensitive)]
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
    use maplit::hashset;
    use primitive_types::H256;
    use secp256k1::{PublicKey, Secp256k1, SecretKey};
    use serde_json::json;
    use web3::signing::keccak256;

    #[test]
    fn deserialization_and_back() {
        let value = json!(
        {
            "creationDate": "1970-01-01T00:00:03Z",
            "owner": "0x0000000000000000000000000000000000000001",
            "uid": "0x1111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111",
            "availableBalance": "100",
            "executedBuyAmount": "3",
            "executedSellAmount": "5",
            "executedSellAmountBeforeFees": "4",
            "executedFeeAmount": "1",
            "invalidated": true,
            "sellToken": "0x000000000000000000000000000000000000000a",
            "buyToken": "0x0000000000000000000000000000000000000009",
            "receiver": "0x000000000000000000000000000000000000000b",
            "sellAmount": "1",
            "buyAmount": "0",
            "validTo": 4294967295u32,
            "appData": "0x6000000000000000000000000000000000000000000000000000000000000007",
            "feeAmount": "115792089237316195423570985008687907853269984665640564039457584007913129639935",
            "kind": "buy",
            "partiallyFillable": false,
            "signature": "0x0200000000000000000000000000000000000000000000000000000000000003040000000000000000000000000000000000000000000000000000000000000501",
            "signingScheme": "eip712",
        });
        let expected = Order {
            order_meta_data: OrderMetaData {
                creation_date: DateTime::<Utc>::from_utc(NaiveDateTime::from_timestamp(3, 0), Utc),
                owner: H160::from_low_u64_be(1),
                uid: OrderUid([17u8; 56]),
                available_balance: Some(100.into()),
                executed_buy_amount: BigUint::from_bytes_be(&[3]),
                executed_sell_amount: BigUint::from_bytes_be(&[5]),
                executed_sell_amount_before_fees: BigUint::from_bytes_be(&[4]),
                executed_fee_amount: BigUint::from_bytes_be(&[1]),
                invalidated: true,
            },
            order_creation: OrderCreation {
                sell_token: H160::from_low_u64_be(10),
                buy_token: H160::from_low_u64_be(9),
                receiver: Some(H160::from_low_u64_be(11)),
                sell_amount: 1.into(),
                buy_amount: 0.into(),
                valid_to: u32::MAX,
                app_data: hex!("6000000000000000000000000000000000000000000000000000000000000007"),
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
                signing_scheme: SigningScheme::Eip712,
            },
        };
        let deserialized: Order = serde_json::from_value(value.clone()).unwrap();
        assert_eq!(deserialized, expected);
        let serialized = serde_json::to_value(expected).unwrap();
        assert_eq!(serialized, value);
    }

    // these two signature tests have been created by printing the order and signature information
    // from the test `should recover signing address for all supported ECDSA-based schemes` in
    // https://github.com/gnosis/gp-v2-contracts/blob/main/test/GPv2Signing.test.ts .
    #[test]
    fn order_creation_signature() {
        let domain_separator = DomainSeparator(hex!(
            "74e0b11bd18120612556bae4578cfd3a254d7e2495f543c569a92ff5794d9b09"
        ));
        let expected_owner = H160(hex!("70997970C51812dc3A010C7d01b50e0d17dc79C8"));
        let expected_uid = OrderUid(hex!("f308e6d59020614692e6d60e53689343e8aa9a3e21670da7e3153aecc5500e6a70997970c51812dc3a010c7d01b50e0d17dc79c8ffffffff"));

        let eip712_signature = hex!("32f1261f1a30c4f9b3e7d17f572ded5c5f4077edce0c105d82c87fd63ae1f9a93bc8e28b1fe390fa45af8217e90c7cf506996c06cdeae9d18f51444e3520d17c1c");
        let ethsign_signature = hex!("cca651a8260b08f318ffd8cd397919368a604836d17322cc4a7ab18eb9d8186e2f73a5b9d6a4a19816771caa776e0a37506d8f3ad2cc2327d4c3709eb66058031c");

        for (signing_scheme, signature) in &[
            (SigningScheme::Eip712, eip712_signature),
            (SigningScheme::EthSign, ethsign_signature),
        ] {
            let order = OrderCreation {
                sell_token: hex!("0101010101010101010101010101010101010101").into(),
                buy_token: hex!("0202020202020202020202020202020202020202").into(),
                receiver: Some(hex!("0303030303030303030303030303030303030303").into()),
                sell_amount: hex!("0246ddf97976680000").as_ref().into(),
                buy_amount: hex!("b98bc829a6f90000").as_ref().into(),
                valid_to: 4294967295,
                app_data: hex!("0000000000000000000000000000000000000000000000000000000000000000"),
                fee_amount: hex!("0de0b6b3a7640000").as_ref().into(),
                kind: OrderKind::Sell,
                partially_fillable: false,
                signature: Signature::from_bytes(&signature),
                signing_scheme: *signing_scheme,
            };

            let uid = order.uid(&domain_separator, &expected_owner);
            assert_eq!(uid, expected_uid);

            let owner = order
                .signature
                .validate(*signing_scheme, &domain_separator, &order.hash_struct())
                .unwrap();
            assert_eq!(owner, expected_owner);
        }
    }

    // from the test `should recover signing address for all supported signing schemes` in
    // https://github.com/gnosis/gp-v2-contracts/blob/main/test/sign.test.ts .
    #[test]
    fn order_cancellation_signature_typed_data() {
        let domain_separator = DomainSeparator(hex!(
            "f8a1143d44c67470a791201b239ff6b0ecc8910aa9682bebd08145f5fd84722b"
        ));

        let expected_owner = H160(hex!("f39Fd6e51aad88F6F4ce6aB8827279cffFb92266"));

        let eip712_signature = hex!("f2c69310a4dbcd78feabfd802df296ca4650681e01872f667251916ed3e9a2e14928382316607594a77c620e4bc4536e6fe145ee993a5ccc38fda929e86830231b");
        let ethsign_signature = hex!("5fef0aed159777403f964da946b2b6c7d2ca6a931f009328c17ed481bf5fe25b46c8da3dfdca2657c9e6e7fbd3a1abbf52ee0ccaf610395fb2445256f5d24eb41b");

        for (signing_scheme, signature) in &[
            (SigningScheme::Eip712, eip712_signature),
            (SigningScheme::EthSign, ethsign_signature),
        ] {
            let cancellation = OrderCancellation {
                order_uid: OrderUid(hex!("2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a")),
                signature: Signature::from_bytes(&signature),
                signing_scheme: *signing_scheme,
            };
            let owner = cancellation.validate(&domain_separator).unwrap();
            assert_eq!(owner, expected_owner);
        }
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

    #[test]
    fn order_contains_token_from() {
        let order = Order::default();
        assert_eq!(
            order.contains_token_from(&hashset!(order.order_creation.sell_token)),
            true
        );
        assert_eq!(
            order.contains_token_from(&hashset!(order.order_creation.buy_token)),
            true
        );
        assert_eq!(order.contains_token_from(&HashSet::new()), false);
        let other_token = H160::from_low_u64_be(1);
        assert_ne!(other_token, order.order_creation.sell_token);
        assert_ne!(other_token, order.order_creation.buy_token);
        assert_eq!(order.contains_token_from(&hashset!(other_token)), false);
    }

    pub fn h160_from_public_key(key: PublicKey) -> H160 {
        let hash = keccak256(&key.serialize_uncompressed()[1..] /* cut '04' */);
        H160::from_slice(&hash[12..])
    }

    #[test]
    fn order_builder_signature_recovery() {
        const PRIVATE_KEY: [u8; 32] =
            hex!("0000000000000000000000000000000000000000000000000000000000000001");
        let sk = SecretKey::from_slice(&PRIVATE_KEY).unwrap();
        let public_key = PublicKey::from_secret_key(&Secp256k1::signing_only(), &sk);
        let order = OrderBuilder::default()
            .with_sell_token(H160::zero())
            .with_sell_amount(100.into())
            .with_buy_token(H160::zero())
            .with_buy_amount(80.into())
            .with_valid_to(u32::max_value())
            .with_kind(OrderKind::Sell)
            .sign_with(&DomainSeparator::default(), SecretKeyRef::from(&sk))
            .build();

        let owner = order
            .order_creation
            .signature
            .validate(
                order.order_creation.signing_scheme,
                &DomainSeparator::default(),
                &order.order_creation.hash_struct(),
            )
            .unwrap();

        assert_eq!(owner, h160_from_public_key(public_key));
    }
}
