//! Chain vocabulary for winner selection.
//!
//! The CIP-38 algorithm is chain-agnostic: it needs identifiers it can hash
//! and compare, amounts it can do checked arithmetic on, and three small
//! chain-specific hooks. Everything else in this crate is written once,
//! generic over [`ChainTypes`].

use std::{fmt::Debug, hash::Hash};

/// The per-chain type vocabulary and hooks.
pub trait ChainTypes: Copy + Debug + Eq + Hash + 'static {
    /// Token identifier (EVM: 20-byte address, Solana: 32-byte mint).
    type TokenId: Copy + Debug + Eq + Hash;
    /// Account identifier, used for solvers and order owners.
    type AccountId: Copy + Debug + Eq + Hash;
    /// Order identifier (EVM: 56-byte UID, Solana: 32-byte intent hash).
    type OrderUid: Copy + Debug + Eq + Hash;
    /// Amount type used for token amounts, prices, and scores.
    type Amount: Amount;

    /// Canonical form of a token for clearing-price uniqueness. EVM maps the
    /// native-token sentinel to the wrapped native token, Solana is identity.
    fn canonical_token(token: Self::TokenId, wrapped_native: Self::TokenId) -> Self::TokenId;

    /// Owner embedded in the order UID, if the chain's UID carries one.
    /// Only used to attribute JIT orders to surplus-capturing owners.
    fn uid_owner(uid: &Self::OrderUid) -> Option<Self::AccountId>;

    /// Convert a token amount to the native token using this price:
    /// `amount * price / native_denominator`.
    fn value_in_native(price: Self::Amount, amount: Self::Amount) -> Self::Amount;
}

/// Checked arithmetic the scoring math needs.
///
/// The mul-div methods mirror how each chain multiplies before dividing:
/// the non-widening variants fail on intermediate overflow (matching the
/// EVM code's `checked_mul` then `checked_div`), the widening variant uses
/// a double-width intermediate and only fails if the final quotient does
/// not fit.
pub trait Amount: Copy + Debug + Default + Ord {
    const ZERO: Self;

    fn is_zero(&self) -> bool {
        *self == Self::ZERO
    }

    fn checked_add(self, rhs: Self) -> Option<Self>;
    fn checked_sub(self, rhs: Self) -> Option<Self>;
    fn saturating_add(self, rhs: Self) -> Self;

    /// `self * mul / div`, rounding down.
    fn mul_div_floor(self, mul: Self, div: Self) -> MathResult<Self>;
    /// `self * mul / div`, rounding up.
    fn mul_div_ceil(self, mul: Self, div: Self) -> MathResult<Self>;
    /// `self * mul / div` with a double-width intermediate, rounding down.
    fn widening_mul_div_floor(self, mul: Self, div: Self) -> MathResult<Self>;
    /// `self * factor` for fee factors in `[0, 1)` ranges.
    fn mul_f64(self, factor: f64) -> Option<Self>;
}

pub type MathResult<T> = Result<T, MathError>;

#[derive(Debug, thiserror::Error)]
pub enum MathError {
    #[error("overflow")]
    Overflow,
    #[error("division by zero")]
    DivisionByZero,
    #[error("negative")]
    Negative,
}
