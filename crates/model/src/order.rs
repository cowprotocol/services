//! Contains the order type as described by the specification with serialization as described by the openapi documentation.

use crate::{
    app_id::AppId,
    interaction::InteractionData,
    quote::QuoteId,
    signature::{EcdsaSignature, EcdsaSigningScheme, Signature, VerificationError},
    u256_decimal::{self, DecimalU256},
    DomainSeparator, TokenPair,
};
use anyhow::{anyhow, Result};
use chrono::{offset::Utc, DateTime};
use derivative::Derivative;
use hex_literal::hex;
use num::BigUint;
use primitive_types::{H160, H256, U256};
use secp256k1::ONE_KEY;
use serde::{de, Deserialize, Deserializer, Serialize, Serializer};
use serde_with::{serde_as, DisplayFromStr};
use std::{
    collections::HashSet,
    fmt::{self, Debug, Display},
    str::FromStr,
};
use strum::{AsRefStr, EnumString, EnumVariantNames};
use web3::signing::{self, Key, SecretKeyRef};

/// The flag denoting that an order is buying ETH (or the chain's native token).
/// It is used in place of an actual buy token address in an order.
pub const BUY_ETH_ADDRESS: H160 = H160([0xee; 20]);

#[derive(Eq, PartialEq, Clone, Debug, Default, Deserialize, Serialize)]
pub struct Interactions {
    pub pre: Vec<InteractionData>,
    // later we can add here intra/post interactions
}

/// An order that is returned when querying the orderbook.
///
/// Contains extra fields that are populated by the orderbook.
#[derive(Eq, PartialEq, Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Order {
    #[serde(flatten)]
    pub metadata: OrderMetadata,
    #[serde(flatten)]
    pub data: OrderData,
    #[serde(flatten)]
    pub signature: Signature,
    #[serde(default)]
    pub interactions: Interactions,
}

#[derive(Eq, PartialEq, Clone, Copy, Debug, Deserialize, Serialize, Hash, Default)]
#[serde(rename_all = "camelCase")]
pub enum OrderStatus {
    PresignaturePending,
    #[default]
    Open,
    Fulfilled,
    Cancelled,
    Expired,
}

impl Order {
    pub fn from_order_creation(
        order: &OrderCreation,
        domain: &DomainSeparator,
        settlement_contract: H160,
        full_fee_amount: U256,
        class: OrderClass,
    ) -> Result<Self, VerificationError> {
        let owner = order.verify_owner(domain)?;
        Ok(Self {
            metadata: OrderMetadata {
                owner,
                creation_date: chrono::offset::Utc::now(),
                uid: order.data.uid(domain, &owner),
                settlement_contract,
                full_fee_amount,
                class,
                ..Default::default()
            },
            signature: order.signature.clone(),
            data: order.data,
            interactions: Interactions::default(),
        })
    }

    pub fn into_order_creation(self) -> OrderCreation {
        self.into()
    }

    pub fn contains_token_from(&self, token_list: &HashSet<H160>) -> bool {
        token_list.contains(&self.data.buy_token) || token_list.contains(&self.data.sell_token)
    }

    pub fn is_user_order(&self) -> bool {
        match self.metadata.class {
            OrderClass::Market | OrderClass::Limit(_) => true,
            OrderClass::Liquidity => false,
        }
    }
}

/// Remaining order buy, sell and fee amounts.
#[derive(Debug, Eq, PartialEq)]
pub struct RemainingOrderAmounts {
    pub sell_amount: U256,
    pub buy_amount: U256,
    pub fee_amount: U256,
    pub full_fee_amount: U256,
}

#[derive(Clone, Default, Debug)]
pub struct OrderBuilder(Order);

impl OrderBuilder {
    pub fn with_sell_token(mut self, sell_token: H160) -> Self {
        self.0.data.sell_token = sell_token;
        self
    }

    pub fn with_buy_token(mut self, buy_token: H160) -> Self {
        self.0.data.buy_token = buy_token;
        self
    }

    pub fn with_sell_amount(mut self, sell_amount: U256) -> Self {
        self.0.data.sell_amount = sell_amount;
        self
    }

    pub fn with_buy_amount(mut self, buy_amount: U256) -> Self {
        self.0.data.buy_amount = buy_amount;
        self
    }

    pub fn with_valid_to(mut self, valid_to: u32) -> Self {
        self.0.data.valid_to = valid_to;
        self
    }

    pub fn with_app_data(mut self, app_data: [u8; 32]) -> Self {
        self.0.data.app_data = AppId(app_data);
        self
    }

    pub fn with_receiver(mut self, receiver: Option<H160>) -> Self {
        self.0.data.receiver = receiver;
        self
    }

    pub fn with_fee_amount(mut self, fee_amount: U256) -> Self {
        self.0.data.fee_amount = fee_amount;
        self
    }

    pub fn with_full_fee_amount(mut self, full_fee_amount: U256) -> Self {
        self.0.metadata.full_fee_amount = full_fee_amount;
        self
    }

    pub fn with_kind(mut self, kind: OrderKind) -> Self {
        self.0.data.kind = kind;
        self
    }

    pub fn with_partially_fillable(mut self, partially_fillable: bool) -> Self {
        self.0.data.partially_fillable = partially_fillable;
        self
    }

    pub fn with_sell_token_balance(mut self, balance: SellTokenSource) -> Self {
        self.0.data.sell_token_balance = balance;
        self
    }

    pub fn with_buy_token_balance(mut self, balance: BuyTokenDestination) -> Self {
        self.0.data.buy_token_balance = balance;
        self
    }

    pub fn with_creation_date(mut self, creation_date: DateTime<Utc>) -> Self {
        self.0.metadata.creation_date = creation_date;
        self
    }

    /// Sets owner, uid, signature.
    pub fn sign_with(
        mut self,
        signing_scheme: EcdsaSigningScheme,
        domain: &DomainSeparator,
        key: SecretKeyRef,
    ) -> Self {
        self.0.metadata.owner = key.address();
        self.0.metadata.uid = self.0.data.uid(domain, &key.address());
        self.0.signature =
            EcdsaSignature::sign(signing_scheme, domain, &self.0.data.hash_struct(), key)
                .to_signature(signing_scheme);
        self
    }

    pub fn with_eip1271(mut self, owner: H160, signature: Vec<u8>) -> Self {
        self.0.metadata.owner = owner;
        self.0.signature = Signature::Eip1271(signature);
        self
    }

    pub fn with_presign(mut self, owner: H160) -> Self {
        self.0.metadata.owner = owner;
        self.0.signature = Signature::PreSign;
        self
    }

    pub fn with_class(mut self, class: OrderClass) -> Self {
        self.0.metadata.class = class;
        self
    }

    pub fn with_surplus_fee(mut self, surplus_fee: U256) -> Self {
        if let OrderClass::Limit(limit) = &mut self.0.metadata.class {
            limit.surplus_fee = surplus_fee;
        } else {
            panic!("not a limit order");
        }
        self
    }

    pub fn build(self) -> Order {
        self.0
    }
}

/// The complete order data.
///
/// These are the exact fields that get signed and verified by the settlement
/// contract.
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OrderData {
    pub sell_token: H160,
    pub buy_token: H160,
    #[serde(default)]
    pub receiver: Option<H160>,
    #[serde(with = "u256_decimal")]
    pub sell_amount: U256,
    #[serde(with = "u256_decimal")]
    pub buy_amount: U256,
    pub valid_to: u32,
    pub app_data: AppId,
    #[serde(with = "u256_decimal")]
    pub fee_amount: U256,
    pub kind: OrderKind,
    pub partially_fillable: bool,
    #[serde(default)]
    pub sell_token_balance: SellTokenSource,
    #[serde(default)]
    pub buy_token_balance: BuyTokenDestination,
}

impl OrderData {
    // See <https://github.com/cowprotocol/contracts/blob/v1.1.2/src/contracts/libraries/GPv2Order.sol#L47>
    pub const TYPE_HASH: [u8; 32] =
        hex!("d5a25ba2e97094ad7d83dc28a6572da797d6b3e7fc6663bd93efb789fc17e489");

    // keccak256("erc20")
    pub const BALANCE_ERC20: [u8; 32] =
        hex!("5a28e9363bb942b639270062aa6bb295f434bcdfc42c97267bf003f272060dc9");
    // keccak256("external")
    pub const BALANCE_EXTERNAL: [u8; 32] =
        hex!("abee3b73373acd583a130924aad6dc38cfdc44ba0555ba94ce2ff63980ea0632");
    // keccak256("internal")
    pub const BALANCE_INTERNAL: [u8; 32] =
        hex!("4ac99ace14ee0a5ef932dc609df0943ab7ac16b7583634612f8dc35a4289a6ce");

    /// Returns the value of hashStruct() over the order data as defined by EIP-712.
    ///
    /// https://eips.ethereum.org/EIPS/eip-712#definition-of-hashstruct
    pub fn hash_struct(&self) -> [u8; 32] {
        let mut hash_data = [0u8; 416];
        hash_data[0..32].copy_from_slice(&Self::TYPE_HASH);
        // Some slots are not assigned (stay 0) because all values are extended to 256 bits.
        hash_data[44..64].copy_from_slice(self.sell_token.as_fixed_bytes());
        hash_data[76..96].copy_from_slice(self.buy_token.as_fixed_bytes());
        hash_data[108..128]
            .copy_from_slice(self.receiver.unwrap_or_else(H160::zero).as_fixed_bytes());
        self.sell_amount.to_big_endian(&mut hash_data[128..160]);
        self.buy_amount.to_big_endian(&mut hash_data[160..192]);
        hash_data[220..224].copy_from_slice(&self.valid_to.to_be_bytes());
        hash_data[224..256].copy_from_slice(&self.app_data.0);
        self.fee_amount.to_big_endian(&mut hash_data[256..288]);
        hash_data[288..320].copy_from_slice(match self.kind {
            OrderKind::Sell => &OrderKind::SELL,
            OrderKind::Buy => &OrderKind::BUY,
        });
        hash_data[351] = self.partially_fillable as u8;
        hash_data[352..384].copy_from_slice(match self.sell_token_balance {
            SellTokenSource::Erc20 => &Self::BALANCE_ERC20,
            SellTokenSource::External => &Self::BALANCE_EXTERNAL,
            SellTokenSource::Internal => &Self::BALANCE_INTERNAL,
        });
        hash_data[384..416].copy_from_slice(match self.buy_token_balance {
            BuyTokenDestination::Erc20 => &Self::BALANCE_ERC20,
            BuyTokenDestination::Internal => &Self::BALANCE_INTERNAL,
        });
        signing::keccak256(&hash_data)
    }

    pub fn token_pair(&self) -> Option<TokenPair> {
        TokenPair::new(self.buy_token, self.sell_token)
    }

    pub fn uid(&self, domain: &DomainSeparator, owner: &H160) -> OrderUid {
        OrderUid::from_parts(
            H256(super::signature::hashed_eip712_message(
                domain,
                &self.hash_struct(),
            )),
            *owner,
            self.valid_to,
        )
    }
}

// An order as provided to the orderbook by the frontend.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OrderCreation {
    #[serde(flatten)]
    pub data: OrderData,
    pub from: Option<H160>,
    #[serde(flatten)]
    pub signature: Signature,
    pub quote_id: Option<QuoteId>,
}

impl OrderCreation {
    /// Recovers the owner address for the specified domain, and then verifies
    /// it matches the expected address.
    ///
    /// Returns the recovered address on success, or an error if there is an
    /// issue performing the EC-recover or the recovered address does not match
    /// the expected one.
    pub fn verify_owner(&self, domain: &DomainSeparator) -> Result<H160, VerificationError> {
        self.signature
            .verify_owner(self.from, domain, &self.data.hash_struct())
    }
}

impl Default for OrderCreation {
    // Custom implementation to make sure the default order creation is valid.
    fn default() -> Self {
        Self {
            data: OrderData {
                valid_to: u32::MAX,
                ..Default::default()
            },
            from: None,
            signature: Signature::Eip712(EcdsaSignature::non_zero()),
            quote_id: None,
        }
    }
}

impl From<Order> for OrderCreation {
    fn from(order: Order) -> Self {
        OrderCreation {
            data: order.data,
            from: Some(order.metadata.owner),
            signature: order.signature,
            quote_id: None,
        }
    }
}

/// Cancellation of multiple orders.
#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize)]
pub struct OrderCancellations {
    pub order_uids: Vec<OrderUid>,
}

impl OrderCancellations {
    /// The EIP-712 type hash for order cancellations. Computed with:
    /// `keccak256("OrderCancellations(bytes[] orderUid)")`.
    const TYPE_HASH: [u8; 32] =
        hex!("4c89efb91ae246f78d2fe68b47db2fa1444a121a4f2dc3fda7a5a408c2e3588e");

    pub fn hash_struct(&self) -> [u8; 32] {
        let mut encoded_uids = Vec::with_capacity(32 * self.order_uids.len());
        for order_uid in &self.order_uids {
            encoded_uids.extend_from_slice(&signing::keccak256(&order_uid.0));
        }

        let array_hash = signing::keccak256(&encoded_uids);

        let mut hash_data = [0u8; 64];
        hash_data[0..32].copy_from_slice(&Self::TYPE_HASH);
        hash_data[32..64].copy_from_slice(&array_hash);
        signing::keccak256(&hash_data)
    }
}

/// Signed order cancellations.
#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize)]
pub struct SignedOrderCancellations {
    #[serde(flatten)]
    pub data: OrderCancellations,
    pub signature: EcdsaSignature,
    pub signing_scheme: EcdsaSigningScheme,
}

impl SignedOrderCancellations {
    pub fn validate(&self, domain_separator: &DomainSeparator) -> Result<H160> {
        self.signature.recover(
            self.signing_scheme,
            domain_separator,
            &self.data.hash_struct(),
        )
    }
}

/// An order cancellation as provided to the orderbook by the frontend.
#[derive(Eq, PartialEq, Clone, Copy, Debug)]
pub struct OrderCancellation {
    pub order_uid: OrderUid,
    pub signature: EcdsaSignature,
    pub signing_scheme: EcdsaSigningScheme,
}

impl Default for OrderCancellation {
    fn default() -> Self {
        Self::for_order(
            OrderUid::default(),
            &DomainSeparator::default(),
            SecretKeyRef::new(&ONE_KEY),
        )
    }
}

// EIP-712
impl OrderCancellation {
    // keccak256("OrderCancellation(bytes orderUid)")
    const TYPE_HASH: [u8; 32] =
        hex!("7b41b3a6e2b3cae020a3b2f9cdc997e0d420643957e7fea81747e984e47c88ec");

    pub fn for_order(
        order_uid: OrderUid,
        domain_separator: &DomainSeparator,
        key: SecretKeyRef,
    ) -> Self {
        let mut result = Self {
            order_uid,
            signature: Default::default(),
            signing_scheme: EcdsaSigningScheme::Eip712,
        };
        result.signature = EcdsaSignature::sign(
            result.signing_scheme,
            domain_separator,
            &result.hash_struct(),
            key,
        );
        result
    }

    pub fn hash_struct(&self) -> [u8; 32] {
        let mut hash_data = [0u8; 64];
        hash_data[0..32].copy_from_slice(&Self::TYPE_HASH);
        hash_data[32..64].copy_from_slice(&signing::keccak256(&self.order_uid.0));
        signing::keccak256(&hash_data)
    }

    pub fn validate(&self, domain_separator: &DomainSeparator) -> Result<H160> {
        self.signature
            .recover(self.signing_scheme, domain_separator, &self.hash_struct())
    }
}

/// Order cancellation payload that is sent over the API.
#[derive(Debug, Eq, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CancellationPayload {
    pub signature: EcdsaSignature,
    pub signing_scheme: EcdsaSigningScheme,
}

#[derive(Debug, PartialEq, Eq, Clone, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EthflowData {
    pub user_valid_to: i64,
    pub is_refunded: bool,
}

/// An order as provided to the orderbook by the frontend.
#[serde_as]
#[derive(Eq, PartialEq, Clone, Default, Derivative, Deserialize, Serialize)]
#[derivative(Debug)]
#[serde(rename_all = "camelCase")]
pub struct OrderMetadata {
    pub creation_date: DateTime<Utc>,
    pub owner: H160,
    pub uid: OrderUid,
    #[serde_as(as = "Option<DecimalU256>")]
    pub available_balance: Option<U256>,
    #[derivative(Debug(format_with = "debug_biguint_to_string"))]
    #[serde_as(as = "DisplayFromStr")]
    pub executed_buy_amount: BigUint,
    #[derivative(Debug(format_with = "debug_biguint_to_string"))]
    #[serde_as(as = "DisplayFromStr")]
    pub executed_sell_amount: BigUint,
    #[serde(default, with = "u256_decimal")]
    pub executed_sell_amount_before_fees: U256,
    #[serde(default, with = "u256_decimal")]
    pub executed_fee_amount: U256,
    pub invalidated: bool,
    pub status: OrderStatus,
    #[serde(flatten)]
    pub class: OrderClass,
    pub settlement_contract: H160,
    #[serde(default, with = "u256_decimal")]
    pub full_fee_amount: U256,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ethflow_data: Option<EthflowData>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub onchain_user: Option<H160>,
    pub is_liquidity_order: bool,
}

// uid as 56 bytes: 32 for orderDigest, 20 for ownerAddress and 4 for validTo
#[derive(Clone, Copy, Eq, Hash, PartialEq, PartialOrd, Ord)]
pub struct OrderUid(pub [u8; 56]);

impl OrderUid {
    /// Intended for easier uid creation in tests.
    pub fn from_integer(i: u32) -> Self {
        let mut uid = OrderUid::default();
        uid.0[0..4].copy_from_slice(&i.to_le_bytes());
        uid
    }

    /// Create a UID from its parts.
    pub fn from_parts(hash: H256, owner: H160, valid_to: u32) -> Self {
        let mut uid = [0; 56];
        uid[0..32].copy_from_slice(hash.as_bytes());
        uid[32..52].copy_from_slice(owner.as_bytes());
        uid[52..56].copy_from_slice(&valid_to.to_be_bytes());
        Self(uid)
    }

    /// Splits an order UID into its parts.
    pub fn parts(&self) -> (H256, H160, u32) {
        (
            H256::from_slice(&self.0[0..32]),
            H160::from_slice(&self.0[32..52]),
            u32::from_le_bytes(self.0[52..].try_into().unwrap()),
        )
    }
}

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

#[derive(Eq, PartialEq, Clone, Copy, Debug, Default, Deserialize, Serialize, Hash, EnumString)]
#[strum(ascii_case_insensitive)]
#[serde(rename_all = "lowercase")]
pub enum OrderKind {
    #[default]
    Buy,
    Sell,
}

#[derive(
    Eq,
    PartialEq,
    Clone,
    Debug,
    Default,
    Deserialize,
    Serialize,
    Hash,
    EnumString,
    AsRefStr,
    EnumVariantNames,
)]
#[strum(ascii_case_insensitive)]
#[serde(tag = "class", rename_all = "lowercase")]
pub enum OrderClass {
    /// The most common type of order which can be placed by any user. Expected to be fulfilled
    /// immediately (in the next block).
    #[default]
    Market,
    /// Liquidity orders can only be placed by whitelisted users. These are
    /// used for matching "coincidence of wants" trades. These are zero-fee orders which are
    /// not expected to be fulfilled immediately and can potentially live for a long time.
    Liquidity,
    /// Orders which are not expected to be fulfilled immediately, but potentially somewhere far in
    /// the future. These are orders where users essentially want to say: "once the price is at least X in
    /// the future, then fulfill my order". These orders have their fee set to zero, because it's
    /// impossible to predict fees that far in the future. Instead, the fee is taken from the order
    /// surplus once the order becomes fulfillable and the surplus is high enough.
    Limit(LimitOrderClass),
}

impl OrderClass {
    pub fn is_limit(&self) -> bool {
        matches!(self, Self::Limit(_))
    }
}

#[serde_as]
#[derive(Eq, PartialEq, Clone, Debug, Default, Hash, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LimitOrderClass {
    #[serde(with = "u256_decimal")]
    pub surplus_fee: U256,
    pub surplus_fee_timestamp: DateTime<Utc>,
    #[serde_as(as = "Option<DecimalU256>")]
    pub executed_surplus_fee: Option<U256>,
}

impl OrderKind {
    // keccak256("sell")
    pub const SELL: [u8; 32] =
        hex!("f3b277728b3fee749481eb3e0b3b48980dbbab78658fc419025cb16eee346775");
    // keccak256("buy")
    pub const BUY: [u8; 32] =
        hex!("6ed88e868af0a1983e3886d5f3e95a2fafbd6c3450bc229e27342283dc429ccc");

    /// Returns a the order kind as a string label that can be used in metrics.
    pub fn label(&self) -> &'static str {
        match self {
            Self::Buy => "buy",
            Self::Sell => "sell",
        }
    }
    pub fn from_contract_bytes(kind: [u8; 32]) -> Result<Self> {
        match kind {
            Self::SELL => Ok(OrderKind::Sell),
            Self::BUY => Ok(OrderKind::Buy),
            _ => Err(anyhow!("Order kind is not well defined")),
        }
    }
}

/// Source from which the sellAmount should be drawn upon order fulfillment
#[derive(Eq, PartialEq, Clone, Copy, Debug, Default, Deserialize, Serialize, Hash, EnumString)]
#[strum(ascii_case_insensitive)]
#[serde(rename_all = "snake_case")]
pub enum SellTokenSource {
    /// Direct ERC20 allowances to the Vault relayer contract
    #[default]
    Erc20,
    /// ERC20 allowances to the Vault with GPv2 relayer approval
    Internal,
    /// Internal balances to the Vault with GPv2 relayer approval
    External,
}

impl SellTokenSource {
    pub fn from_contract_bytes(bytes: [u8; 32]) -> Result<Self> {
        match bytes {
            OrderData::BALANCE_INTERNAL => Ok(Self::Internal),
            OrderData::BALANCE_EXTERNAL => Ok(Self::External),
            OrderData::BALANCE_ERC20 => Ok(Self::Erc20),
            _ => Err(anyhow!("Order sellTokenSource is not well defined")),
        }
    }
}

/// Destination for which the buyAmount should be transferred to order's receiver to upon fulfillment
#[derive(Eq, PartialEq, Clone, Copy, Debug, Default, Deserialize, Serialize, Hash, EnumString)]
#[strum(ascii_case_insensitive)]
#[serde(rename_all = "snake_case")]
pub enum BuyTokenDestination {
    /// Pay trade proceeds as an ERC20 token transfer
    #[default]
    Erc20,
    /// Pay trade proceeds as a Vault internal balance transfer
    Internal,
}

impl BuyTokenDestination {
    pub fn from_contract_bytes(bytes: [u8; 32]) -> Result<Self> {
        match bytes {
            OrderData::BALANCE_INTERNAL => Ok(Self::Internal),
            OrderData::BALANCE_ERC20 => Ok(Self::Erc20),
            _ => Err(anyhow!("Order buyTokenDestination is not well defined")),
        }
    }
}

pub fn debug_app_data(
    app_data: &[u8; 32],
    formatter: &mut std::fmt::Formatter,
) -> Result<(), std::fmt::Error> {
    formatter.write_fmt(format_args!("{:?}", H256(*app_data)))
}

pub fn debug_biguint_to_string(
    value: &BigUint,
    formatter: &mut std::fmt::Formatter,
) -> Result<(), std::fmt::Error> {
    formatter.write_fmt(format_args!("{}", value))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::signature::{EcdsaSigningScheme, SigningScheme};
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
            "surplusFee": "115792089237316195423570985008687907853269984665640564039457584007913129639935",
            "surplusFeeTimestamp": "1970-01-01T00:00:00Z",
            "executedSurplusFee": "1",
            "fullFeeAmount": "115792089237316195423570985008687907853269984665640564039457584007913129639935",
            "kind": "buy",
            "class": "limit",
            "partiallyFillable": false,
            "signature": "0x0200000000000000000000000000000000000000000000000000000000000003040000000000000000000000000000000000000000000000000000000000000501",
            "signingScheme": "eip712",
            "status": "open",
            "settlementContract": "0x0000000000000000000000000000000000000002",
            "sellTokenBalance": "external",
            "buyTokenBalance": "internal",
            "isLiquidityOrder": false,
            "interactions": {
                    "pre": []
            }
        });
        let signing_scheme = EcdsaSigningScheme::Eip712;
        let expected = Order {
            metadata: OrderMetadata {
                creation_date: DateTime::<Utc>::from_utc(NaiveDateTime::from_timestamp(3, 0), Utc),
                class: OrderClass::Limit(LimitOrderClass {
                    surplus_fee: U256::MAX,
                    surplus_fee_timestamp: Default::default(),
                    executed_surplus_fee: Some(1.into()),
                }),
                owner: H160::from_low_u64_be(1),
                uid: OrderUid([17u8; 56]),
                available_balance: Some(100.into()),
                executed_buy_amount: BigUint::from_bytes_be(&[3]),
                executed_sell_amount: BigUint::from_bytes_be(&[5]),
                executed_sell_amount_before_fees: 4.into(),
                executed_fee_amount: 1.into(),
                invalidated: true,
                status: OrderStatus::Open,
                settlement_contract: H160::from_low_u64_be(2),
                full_fee_amount: U256::MAX,
                ..Default::default()
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
            interactions: Interactions::default(),
        };
        let deserialized: Order = serde_json::from_value(value.clone()).unwrap();
        assert_eq!(deserialized, expected);
        let serialized = serde_json::to_value(expected).unwrap();
        assert_eq!(serialized, value);
    }

    #[test]
    fn order_creation_serialization() {
        let owner = H160([0xff; 20]);
        for (signature, signing_scheme, from, signature_bytes) in [
            (
                Signature::default_with(SigningScheme::Eip712),
                "eip712",
                Some(owner),
                "0x0000000000000000000000000000000000000000000000000000000000000000\
                   0000000000000000000000000000000000000000000000000000000000000000\
                   00",
            ),
            (
                Signature::default_with(SigningScheme::EthSign),
                "ethsign",
                None,
                "0x0000000000000000000000000000000000000000000000000000000000000000\
                   0000000000000000000000000000000000000000000000000000000000000000\
                   00",
            ),
            (Signature::PreSign, "presign", Some(owner), "0x"),
        ] {
            let order = OrderCreation {
                data: OrderData {
                    sell_token: H160([0x11; 20]),
                    buy_token: H160([0x22; 20]),
                    receiver: Some(H160([0x33; 20])),
                    sell_amount: 123.into(),
                    buy_amount: 456.into(),
                    valid_to: 1337,
                    app_data: AppId([0x44; 32]),
                    fee_amount: 789.into(),
                    kind: OrderKind::Sell,
                    partially_fillable: false,
                    sell_token_balance: SellTokenSource::Erc20,
                    buy_token_balance: BuyTokenDestination::Erc20,
                },
                from,
                signature,
                quote_id: Some(42),
            };
            let order_json = json!({
                "sellToken": "0x1111111111111111111111111111111111111111",
                "buyToken": "0x2222222222222222222222222222222222222222",
                "receiver": "0x3333333333333333333333333333333333333333",
                "sellAmount": "123",
                "buyAmount": "456",
                "validTo": 1337,
                "appData": "0x4444444444444444444444444444444444444444444444444444444444444444",
                "feeAmount": "789",
                "kind": "sell",
                "partiallyFillable": false,
                "sellTokenBalance": "erc20",
                "buyTokenBalance": "erc20",
                "quoteId": 42,
                "signingScheme": signing_scheme,
                "signature": signature_bytes,
                "from": from,
            });

            assert_eq!(json!(order), order_json);
            assert_eq!(order, serde_json::from_value(order_json).unwrap());
        }
    }

    // from the test `should recover signing address for all supported ECDSA-based schemes` in
    // <https://github.com/cowprotocol/contracts/blob/v1.1.2/test/GPv2Signing.test.ts#L280>.
    #[test]
    fn order_creation_signature() {
        let domain_separator = DomainSeparator(hex!(
            "74e0b11bd18120612556bae4578cfd3a254d7e2495f543c569a92ff5794d9b09"
        ));
        let expected_owner = H160(hex!("70997970C51812dc3A010C7d01b50e0d17dc79C8"));

        for (signing_scheme, signature) in &[
            (
                SigningScheme::Eip712,
                hex!(
                    "59c0f5c151071c1320575f6da826a6c276525bbe733234bad1afb2879657d65d
                     2afe6812746f4cc97f28f3a5dfdbfc7087511695d23da5e9792cd7ed6c9ddeb7
                     1c"
                ),
            ),
            (
                SigningScheme::EthSign,
                hex!(
                    "bf3bc5a9b60d08dc05768320553ba59a6f301d985903618cfc002e8a61cb78b5
                     5d4a474a43a60193d90cda35ff2764f3913b47e5b5b87770064f549fe871afcc
                     1b"
                ),
            ),
        ] {
            let order = OrderData {
                sell_token: hex!("0101010101010101010101010101010101010101").into(),
                buy_token: hex!("0202020202020202020202020202020202020202").into(),
                receiver: Some(hex!("0303030303030303030303030303030303030303").into()),
                sell_amount: 0x0246ddf97976680000_u128.into(),
                buy_amount: 0xb98bc829a6f90000_u128.into(),
                valid_to: 0xffffffff,
                app_data: AppId(hex!(
                    "0000000000000000000000000000000000000000000000000000000000000000"
                )),
                fee_amount: 0x0de0b6b3a7640000_u128.into(),
                kind: OrderKind::Sell,
                partially_fillable: false,
                sell_token_balance: SellTokenSource::Erc20,
                buy_token_balance: BuyTokenDestination::Erc20,
            };
            let signature = Signature::from_bytes(*signing_scheme, signature).unwrap();

            let owner = signature
                .recover(&domain_separator, &order.hash_struct())
                .unwrap()
                .unwrap();
            assert_eq!(owner, expected_owner);
        }
    }

    // from the test `should compute order unique identifier` in
    // <https://github.com/cowprotocol/contracts/blob/v1.1.2/test/GPv2Signing.test.ts#L143>
    #[test]
    fn compute_order_uid() {
        let domain_separator = DomainSeparator(hex!(
            "74e0b11bd18120612556bae4578cfd3a254d7e2495f543c569a92ff5794d9b09"
        ));
        let owner = hex!("70997970C51812dc3A010C7d01b50e0d17dc79C8").into();
        let order = OrderData {
            sell_token: hex!("0101010101010101010101010101010101010101").into(),
            buy_token: hex!("0202020202020202020202020202020202020202").into(),
            receiver: Some(hex!("0303030303030303030303030303030303030303").into()),
            sell_amount: 0x0246ddf97976680000_u128.into(),
            buy_amount: 0xb98bc829a6f90000_u128.into(),
            valid_to: 0xffffffff,
            app_data: AppId(hex!(
                "0000000000000000000000000000000000000000000000000000000000000000"
            )),
            fee_amount: 0x0de0b6b3a7640000_u128.into(),
            kind: OrderKind::Sell,
            partially_fillable: false,
            sell_token_balance: SellTokenSource::Erc20,
            buy_token_balance: BuyTokenDestination::Erc20,
        };

        assert_eq!(
            order.uid(&domain_separator, &owner).0,
            hex!(
                "0e45d31fd31b28c26031cdd81b35a8938b2ccca2cc425fcf440fd3bfed1eede9
                 70997970c51812dc3a010c7d01b50e0d17dc79c8
                 ffffffff"
            ),
        );
    }

    #[test]
    fn order_cancellation_signature_typed_data() {
        let domain_separator = DomainSeparator(hex!(
            "f8a1143d44c67470a791201b239ff6b0ecc8910aa9682bebd08145f5fd84722b"
        ));

        let expected_owner = H160(hex!("f39Fd6e51aad88F6F4ce6aB8827279cffFb92266"));

        let eip712_signature = hex!("f2c69310a4dbcd78feabfd802df296ca4650681e01872f667251916ed3e9a2e14928382316607594a77c620e4bc4536e6fe145ee993a5ccc38fda929e86830231b");
        let ethsign_signature = hex!("5fef0aed159777403f964da946b2b6c7d2ca6a931f009328c17ed481bf5fe25b46c8da3dfdca2657c9e6e7fbd3a1abbf52ee0ccaf610395fb2445256f5d24eb41b");

        for (signing_scheme, signature) in &[
            (EcdsaSigningScheme::Eip712, eip712_signature),
            (EcdsaSigningScheme::EthSign, ethsign_signature),
        ] {
            let cancellation = OrderCancellation {
                order_uid: OrderUid(hex!("2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a")),
                signature: EcdsaSignature::from_bytes(signature),
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
        assert!(order.contains_token_from(&hashset!(order.data.sell_token)),);
        assert!(order.contains_token_from(&hashset!(order.data.buy_token)),);
        assert!(!order.contains_token_from(&HashSet::new()));
        let other_token = H160::from_low_u64_be(1);
        assert_ne!(other_token, order.data.sell_token);
        assert_ne!(other_token, order.data.buy_token);
        assert!(!order.contains_token_from(&hashset!(other_token)));
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
            .with_valid_to(u32::MAX)
            .with_app_data([1u8; 32])
            .with_fee_amount(U256::from(1337))
            .with_partially_fillable(true)
            .with_sell_token_balance(SellTokenSource::External)
            .with_buy_token_balance(BuyTokenDestination::Internal)
            .with_creation_date(DateTime::<Utc>::from_utc(
                NaiveDateTime::from_timestamp(3, 0),
                Utc,
            ))
            .with_presign(H160::from_low_u64_be(1))
            .with_kind(OrderKind::Sell)
            .sign_with(
                EcdsaSigningScheme::Eip712,
                &DomainSeparator::default(),
                SecretKeyRef::from(&sk),
            )
            .build();

        let owner = order
            .signature
            .recover(&DomainSeparator::default(), &order.data.hash_struct())
            .unwrap()
            .unwrap();

        assert_eq!(owner, h160_from_public_key(public_key));
    }

    #[test]
    fn debug_order_data() {
        dbg!(Order::default());
    }

    #[test]
    fn order_cancellations_struct_hash() {
        // Generated with Ethers.js as a reference EIP-712 hashing impl.
        for (order_uids, struct_hash) in [
            (
                vec![],
                hex!("56acdb3034898c6c23971cb3f92c32a4739e89a13c85282547025583a93911bd"),
            ),
            (
                vec![OrderUid([0x11; 56]), OrderUid([0x22; 56])],
                hex!("405f6cb53d87901a5385a824a99c94b43146547f5ea3623f8d2f50b925e97a8b"),
            ),
        ] {
            let cancellations = OrderCancellations { order_uids };
            assert_eq!(cancellations.hash_struct(), struct_hash);
        }
    }
}
