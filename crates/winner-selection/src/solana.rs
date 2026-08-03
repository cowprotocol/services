//! Solana instantiation of the chain vocabulary.
//!
//! Deliberately free of any Solana SDK dependency: the algorithm only needs
//! identifiers it can hash and compare, so 32-byte newtypes suffice.

use crate::chain::{Amount, ChainTypes, MathError, MathResult};

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

    /// Identity: orders always trade SPL token accounts, native SOL enters
    /// settlement pre-wrapped as the wSOL mint, so there is no sentinel to
    /// canonicalize.
    fn canonical_token(token: Pubkey, _wrapped_native: Pubkey) -> Pubkey {
        token
    }

    /// The intent hash embeds no owner. JIT surplus capture is unsupported
    /// on Solana, so no caller misses it.
    fn uid_owner(_uid: &IntentHash) -> Option<Pubkey> {
        None
    }

    /// `amount * price / 10^9` with a u128 intermediate, saturating to
    /// `u64::MAX` like the EVM path saturates its multiplication.
    fn value_in_native(price: u64, amount: u64) -> u64 {
        let wide = u128::from(price) * u128::from(amount) / NATIVE_DENOMINATOR;
        u64::try_from(wide).unwrap_or(u64::MAX)
    }
}

/// u64 amounts always use u128 intermediates: every u64 product fits, so
/// unlike the EVM's U256 the non-widening and widening variants coincide.
impl Amount for u64 {
    const ZERO: Self = 0;

    fn checked_add(self, rhs: Self) -> Option<Self> {
        u64::checked_add(self, rhs)
    }

    fn checked_sub(self, rhs: Self) -> Option<Self> {
        u64::checked_sub(self, rhs)
    }

    fn saturating_add(self, rhs: Self) -> Self {
        u64::saturating_add(self, rhs)
    }

    fn mul_div_floor(self, mul: Self, div: Self) -> MathResult<Self> {
        if div == 0 {
            return Err(MathError::DivisionByZero);
        }
        let wide = u128::from(self) * u128::from(mul) / u128::from(div);
        u64::try_from(wide).map_err(|_| MathError::Overflow)
    }

    fn mul_div_ceil(self, mul: Self, div: Self) -> MathResult<Self> {
        if div == 0 {
            return Err(MathError::DivisionByZero);
        }
        let wide = (u128::from(self) * u128::from(mul)).div_ceil(u128::from(div));
        u64::try_from(wide).map_err(|_| MathError::Overflow)
    }

    fn widening_mul_div_floor(self, mul: Self, div: Self) -> MathResult<Self> {
        self.mul_div_floor(mul, div)
    }

    fn mul_f64(self, factor: f64) -> Option<Self> {
        if !factor.is_finite() || factor < 0.0 {
            return None;
        }
        let result = self as f64 * factor;
        (result <= u64::MAX as f64).then_some(result as u64)
    }
}
