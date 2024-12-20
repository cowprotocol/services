//! The domain object representing a CoW Protocol order.

use {
    crate::{domain::eth, util},
    ethereum_types::{Address, H256},
    std::fmt::{self, Debug, Display, Formatter},
};

/// A CoW Protocol order in the auction.
#[derive(Debug, Clone)]
pub struct Order {
    pub uid: Uid,
    pub sell: eth::Asset,
    pub buy: eth::Asset,
    pub side: Side,
    pub class: Class,
    pub partially_fillable: bool,
}

impl Order {
    /// Returns `true` if the order expects a solver-computed fee.
    pub fn solver_determines_fee(&self) -> bool {
        self.class == Class::Limit
    }
}

/// UID of an order.
#[derive(Clone, Copy, Eq, Hash, PartialEq)]
pub struct Uid(pub [u8; 56]);

impl Debug for Uid {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        f.debug_tuple("Uid")
            .field(&util::fmt::Hex(&self.0))
            .finish()
    }
}

impl Display for Uid {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        Display::fmt(&util::fmt::Hex(&self.0), f)
    }
}

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
}

/// An order that can be used to provide just-in-time liquidity in form of a CoW
/// Protocol order. This is how solvers integrate private market makers into
/// their solutions.
#[derive(Debug)]
pub struct JitOrder {
    pub owner: Address,
    pub signature: Signature,
    pub sell: eth::Asset,
    pub buy: eth::Asset,
    pub side: Side,
    pub class: Class,
    pub partially_fillable: bool,
    pub valid_to: u32,
    pub app_data: AppData,
    pub receiver: Address,
}

/// Signature over the order data.
/// All variants rely on the EIP-712 hash of the order data, referred to as the
/// order hash.
#[derive(Debug, Clone)]
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

/// This is a hash allowing arbitrary user data to be associated with an order.
/// While this type holds the hash, the data itself is uploaded to IPFS. This
/// hash is signed along with the order.
#[derive(Clone, Copy, Default)]
pub struct AppData(pub [u8; 32]);

impl Debug for AppData {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.debug_tuple("AppData")
            .field(&util::fmt::Hex(&self.0))
            .finish()
    }
}
