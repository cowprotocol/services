//! EVM instantiation of the chain vocabulary.

pub use alloy_primitives::{Address, U256};
use {
    crate::chain::{Amount, ChainTypes, MathError, MathResult},
    alloy_primitives::{U512, ruint::UintTryFrom},
    number::u256_ext::U256Ext,
};

/// Native token constant.
pub const NATIVE_TOKEN: Address = Address::repeat_byte(0xee);

/// If the token is native, return the ERC20 wrapped version.
pub fn as_erc20(token: Address, wrapped: Address) -> Address {
    if token == NATIVE_TOKEN {
        wrapped
    } else {
        token
    }
}

/// Convert a token amount to ETH using this price.
///
/// Formula: `amount * price / 10^18`
pub fn price_in_eth(price: U256, amount: U256) -> U256 {
    amount.saturating_mul(price) / U256::from(1_000_000_000_000_000_000u64)
}

/// A unique identifier for an order.
///
/// This is a 56-byte array consisting of:
/// - 32 bytes: order digest (hash of order parameters)
/// - 20 bytes: owner address
/// - 4 bytes: valid until timestamp
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct OrderUid(pub [u8; 56]);

impl OrderUid {
    /// Extract the owner address from the order UID.
    pub fn owner(&self) -> Address {
        // Bytes 32-51 contain the owner address (20 bytes)
        let mut bytes = [0u8; 20];
        bytes.copy_from_slice(&self.0[32..52]);
        Address::from(bytes)
    }
}

/// The EVM chain marker.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct Evm;

impl ChainTypes for Evm {
    type AccountId = Address;
    type Amount = U256;
    type OrderUid = OrderUid;
    type TokenId = Address;

    fn canonical_token(token: Address, wrapped_native: Address) -> Address {
        as_erc20(token, wrapped_native)
    }

    fn uid_owner(uid: &OrderUid) -> Option<Address> {
        Some(uid.owner())
    }

    fn value_in_native(price: U256, amount: U256) -> U256 {
        price_in_eth(price, amount)
    }
}

impl Amount for U256 {
    const ZERO: Self = U256::ZERO;

    fn checked_add(self, rhs: Self) -> Option<Self> {
        U256::checked_add(self, rhs)
    }

    fn checked_sub(self, rhs: Self) -> Option<Self> {
        U256::checked_sub(self, rhs)
    }

    fn saturating_add(self, rhs: Self) -> Self {
        U256::saturating_add(self, rhs)
    }

    fn mul_div_floor(self, mul: Self, div: Self) -> MathResult<Self> {
        self.checked_mul(mul)
            .ok_or(MathError::Overflow)?
            .checked_div(div)
            .ok_or(MathError::DivisionByZero)
    }

    fn mul_div_ceil(self, mul: Self, div: Self) -> MathResult<Self> {
        self.checked_mul(mul)
            .ok_or(MathError::Overflow)?
            .checked_ceil_div(&div)
            .ok_or(MathError::DivisionByZero)
    }

    fn widening_mul_div_floor(self, mul: Self, div: Self) -> MathResult<Self> {
        let wide = self
            .widening_mul(mul)
            .checked_div(U512::from(div))
            .ok_or(MathError::DivisionByZero)?;
        U256::uint_try_from(wide).map_err(|_| MathError::Overflow)
    }

    fn mul_f64(self, factor: f64) -> Option<Self> {
        self.checked_mul_f64(factor)
    }
}
