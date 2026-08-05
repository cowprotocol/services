//! Solana instantiation of the chain vocabulary.
//!
//! No solana-sdk dependency: the algorithm only needs identifiers it can
//! hash and compare, 32-byte newtypes suffice.

use crate::{Amount, ChainTypes, MathError, MathResult};

/// A Solana account address (token mint or solver identity).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Pubkey(pub [u8; 32]);

/// A Solana order identifier: the 32-byte intent hash. Unlike the EVM
/// 56-byte UID it carries no embedded owner.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct IntentHash(pub [u8; 32]);

/// Lamports per SOL, the native-price denominator (9 decimals).
const NATIVE_DENOMINATOR: u128 = 1_000_000_000;

/// The Solana chain marker.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct Solana;

impl ChainTypes for Solana {
    type AccountId = Pubkey;
    type Amount = u64;
    type OrderUid = IntentHash;
    type TokenId = Pubkey;

    /// Identity: Solana orders name SPL mints directly, there is no
    /// native-token sentinel to map.
    fn canonical_token(token: Pubkey, _wrapped_native: Pubkey) -> Pubkey {
        token
    }

    /// The intent hash embeds no owner, so JIT orders cannot be attributed.
    fn uid_owner(_uid: &IntentHash) -> Option<Pubkey> {
        None
    }

    /// `amount * price / 10^9` with a u128 intermediate, saturating to
    /// `u64::MAX` on overflow.
    fn value_in_native(price: u64, amount: u64) -> u64 {
        let wide = u128::from(price) * u128::from(amount) / NATIVE_DENOMINATOR;
        u64::try_from(wide).unwrap_or(u64::MAX)
    }
}

/// u64 amounts always use u128 intermediates: every u64 product fits, so
/// unlike the EVM's U256 the non-widening and widening variants coincide.
impl Amount for u64 {
    fn try_mul_div_floor(self, mul: Self, div: Self) -> MathResult<Self> {
        if div == 0 {
            return Err(MathError::DivisionByZero);
        }
        let wide = u128::from(self) * u128::from(mul) / u128::from(div);
        u64::try_from(wide).map_err(|_| MathError::Overflow)
    }

    fn try_mul_div_ceil(self, mul: Self, div: Self) -> MathResult<Self> {
        if div == 0 {
            return Err(MathError::DivisionByZero);
        }
        let wide = (u128::from(self) * u128::from(mul)).div_ceil(u128::from(div));
        u64::try_from(wide).map_err(|_| MathError::Overflow)
    }

    fn try_widening_mul_div_floor(self, mul: Self, div: Self) -> MathResult<Self> {
        self.try_mul_div_floor(mul, div)
    }

    fn try_mul_f64(self, factor: f64) -> MathResult<Self> {
        if !factor.is_finite() || factor < 0.0 {
            return Err(MathError::Overflow);
        }
        let result = self as f64 * factor;
        (result <= u64::MAX as f64)
            .then_some(result as u64)
            .ok_or(MathError::Overflow)
    }
}
