//! The domain object representing a CoW Protocol order.

use {
    crate::domain::eth,
    ethereum_types::{Address, H256, U256},
};

/// A CoW Protocol order in the auction.
#[derive(Debug, Clone)]
pub struct Order {
    pub uid: Uid,
    pub sell: eth::Asset,
    pub buy: eth::Asset,
    pub fee: Fee,
    pub side: Side,
    pub class: Class,
    pub partially_fillable: bool,
    pub reward: Reward,
}

impl Order {
    /// Returns the order's fee amount as an asset.
    pub fn fee(&self) -> eth::Asset {
        eth::Asset {
            token: self.sell.token,
            amount: self.fee.0,
        }
    }
}

/// UID of an order.
#[derive(Clone, Copy, Debug)]
pub struct Uid(pub [u8; 56]);

/// An order fee amount, denominated in its sell token.
#[derive(Clone, Copy, Debug)]
pub struct Fee(pub U256);

/// The trading side of an order.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Side {
    /// An order with a fixed buy amount and maximum sell amount.
    Buy,
    /// An order with a fixed sell amount and a minimum buy amount.
    Sell,
}

/// The order classification.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Class {
    Market,
    Limit,
    Liquidity,
}

/// An order that is guaranteed to not be a liquidity order.
///
/// Note that the concept of a "non-liquidity" order is important enough to
/// merit its own type. The reason for this is that these orders and liquidity
/// orders differ in fundamental ways and we do not want to confuse them and
/// accidentally use a liquidity order where it shouldn't be used. Some of the
/// notable differences between the order types are:
///
/// - Liquidity orders can't be settled directly against on-chain liquidity.
///   They are meant to only be used in CoWs to facilitate the trading of other
///   non-liquidity orders.
/// - Liquidity orders do no provide any solver rewards
///
/// As their name suggests, they are meant as a mechanism for providing
/// liquidity on CoW Protocol to other non-liquidity orders: they provide a
/// mechanism for turning one token into another. In this regard, a liquidity
/// order is conceptually similar to `liquidity::Liquidity`. One notable
/// difference between the two is in how they are executed. General liquidity
/// requires tokens up-front in order to exchange them for something else. On
/// the other hand, liquidity orders are CoW Protocol orders, meaning that they
/// first provide the tokens being swapped to and only get paid at the end of
/// the settlement.
#[derive(Debug)]
pub struct NonLiquidity<'a>(&'a Order);

impl<'a> NonLiquidity<'a> {
    /// Wraps an order as a user order, returns `None` if the specified order is
    /// not a user order.
    pub fn new(order: &'a Order) -> Option<Self> {
        match order.class {
            Class::Market | Class::Limit => Some(Self(order)),
            Class::Liquidity => None,
        }
    }

    /// Returns a reference to the underlying CoW Protocol order.
    pub fn get(&self) -> &'a Order {
        self.0
    }
}

/// A COW reward amount, in base units.
#[derive(Clone, Copy, Debug)]
pub struct Reward(pub f64);

#[derive(Debug, Clone)]
pub struct CustomInteraction {
    pub target: Address,
    pub value: eth::Ether,
    pub calldata: Vec<u8>,
}

pub struct JitOrder {
    pub owner: Address,
    pub signature: Signature,
    pub sell: eth::Asset,
    pub buy: eth::Asset,
    pub fee: Fee,
    pub side: Side,
    pub class: Class,
    pub partially_fillable: bool,
    pub pre_interactions: Vec<CustomInteraction>,
}

/// Signature over the order data.
/// All variants rely on the EIP-712 hash of the order data, referred to as the
/// order hash.
#[derive(Clone)]
pub enum Signature {
    /// The order struct is signed according to EIP-712.
    ///
    /// https://eips.ethereum.org/EIPS/eip-712
    Eip712(EcdsaSignature),
    /// The order hash is signed according to EIP-191's personal_sign signature
    /// format.
    ///
    /// https://eips.ethereum.org/EIPS/eip-191
    EthSign(EcdsaSignature),
    /// Signature verified according to EIP-1271, which facilitates a way for
    /// contracts to verify signatures using an arbitrary method. This
    /// allows smart contracts to sign and place orders. The order hash is
    /// passed to the verification method, along with this signature.
    ///
    /// https://eips.ethereum.org/EIPS/eip-1271
    Eip1271(Vec<u8>),
    /// For these signatures, the user broadcasts a transaction onchain. This
    /// transaction contains a signature of the order hash. Because this
    /// onchain transaction is also signed, it proves that the user indeed
    /// signed the order.
    PreSign,
}

impl Signature {
    #[allow(clippy::wrong_self_convention)]
    pub fn to_bytes(&self) -> Vec<u8> {
        match self {
            Self::Eip712(signature) | Self::EthSign(signature) => signature.to_bytes().to_vec(),
            Self::Eip1271(signature) => signature.clone(),
            Self::PreSign => Vec::new(),
        }
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub struct EcdsaSignature {
    pub r: H256,
    pub s: H256,
    pub v: u8,
}

impl EcdsaSignature {
    pub fn to_bytes(self) -> [u8; 65] {
        let mut bytes = [0u8; 65];
        bytes[..32].copy_from_slice(self.r.as_bytes());
        bytes[32..64].copy_from_slice(self.s.as_bytes());
        bytes[64] = self.v;
        bytes
    }
}
