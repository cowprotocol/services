//! EVM instantiation of the chain vocabulary.

pub use alloy_primitives::{Address, U256};
use {
    crate::{Amount, ChainTypes, MathError, MathResult, Scoring},
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
    type OrderUid = OrderUid;
}

impl Scoring for Evm {
    type Amount = U256;
    type TokenId = Address;

    const NATIVE_PRICE_DENOMINATOR: U256 = U256::from_limbs([10u64.pow(18), 0, 0, 0]);

    fn canonical_token(token: Address, wrapped_native: Address) -> Address {
        as_erc20(token, wrapped_native)
    }

    fn uid_owner(uid: &OrderUid) -> Option<Address> {
        Some(uid.owner())
    }
}

impl Amount for U256 {
    fn try_mul_div_floor(self, mul: Self, div: Self) -> MathResult<Self> {
        self.checked_mul(mul)
            .ok_or(MathError::Overflow)?
            .checked_div(div)
            .ok_or(MathError::DivisionByZero)
    }

    fn try_mul_div_ceil(self, mul: Self, div: Self) -> MathResult<Self> {
        self.checked_mul(mul)
            .ok_or(MathError::Overflow)?
            .checked_ceil_div(&div)
            .ok_or(MathError::DivisionByZero)
    }

    fn try_widening_mul_div_floor(self, mul: Self, div: Self) -> MathResult<Self> {
        let wide = self
            .widening_mul(mul)
            .checked_div(U512::from(div))
            .ok_or(MathError::DivisionByZero)?;
        U256::uint_try_from(wide).map_err(|_| MathError::Overflow)
    }

    fn try_mul_f64(self, factor: f64) -> MathResult<Self> {
        self.checked_mul_f64(factor).ok_or(MathError::Overflow)
    }
}
