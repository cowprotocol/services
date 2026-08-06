//! Per-chain type vocabulary for chain-generic protocol logic.
//!
//! Chain-generic algorithms need identifiers they can hash and compare,
//! amounts they can do checked arithmetic on, and a small set of
//! chain-specific hooks. This crate defines that vocabulary ([`ChainTypes`],
//! [`Amount`]) together with its EVM and Solana instantiations.

pub mod evm;
pub mod solana;

use std::{fmt::Debug, hash::Hash};

pub use num::traits::{Bounded, CheckedAdd, CheckedSub, SaturatingAdd, Zero};

/// The per-chain type vocabulary and hooks.
pub trait ChainTypes: Copy + Debug + Eq + Hash + Send + Sync + 'static {
    /// Token identifier (EVM: 20-byte address, Solana: 32-byte mint).
    type TokenId: Copy + Debug + Eq + Hash + Send + Sync;
    /// Account identifier, used for solvers and order owners.
    type AccountId: Copy + Debug + Eq + Hash + Send + Sync;
    /// Order identifier (EVM: 56-byte UID, Solana: 32-byte intent hash).
    type OrderUid: Copy + Debug + Eq + Hash + Send + Sync;
    /// Amount type used for token amounts, prices, and scores.
    type Amount: Amount;

    /// Canonical form of a token for clearing-price uniqueness. EVM maps the
    /// native-token sentinel (a buy-side-only value, sell tokens arrive
    /// wrapped) to the wrapped native token, Solana is identity.
    fn canonical_token(token: Self::TokenId, wrapped_native: Self::TokenId) -> Self::TokenId;

    /// Owner embedded in the order UID, if the chain's UID carries one.
    /// Only used to attribute JIT orders to surplus-capturing owners.
    fn uid_owner(uid: &Self::OrderUid) -> Option<Self::AccountId>;

    /// What one whole native token is priced in: the scale of the chain's
    /// native prices (EVM: 10^18 wei, Solana: 10^9 lamports).
    const NATIVE_PRICE_DENOMINATOR: Self::Amount;

    /// Convert a token amount to the native token using this price:
    /// `amount * price / NATIVE_PRICE_DENOMINATOR` with a widening
    /// intermediate, saturating when the result does not fit the amount type.
    fn value_in_native(price: Self::Amount, amount: Self::Amount) -> Self::Amount {
        amount
            .try_widening_mul_div_floor(price, Self::NATIVE_PRICE_DENOMINATOR)
            .unwrap_or_else(|_| Self::Amount::max_value())
    }
}

/// Checked arithmetic the scoring math needs.
///
/// Plain arithmetic comes from the `num` traits. The mul-div combinations and
/// fee scaling are domain operations `num` does not model: the non-widening
/// mul-div variants fail on intermediate overflow, the widening variant uses
/// a double-width intermediate and only fails if the final quotient does not
/// fit.
pub trait Amount:
    Copy + Debug + Ord + Send + Sync + Zero + Bounded + CheckedAdd + CheckedSub + SaturatingAdd
{
    /// `self + rhs`, `Overflow` when it does not fit.
    fn try_add(self, rhs: Self) -> MathResult<Self> {
        self.checked_add(&rhs).ok_or(MathError::Overflow)
    }

    /// `self - rhs`, `Negative` when the result would go below zero.
    fn try_sub(self, rhs: Self) -> MathResult<Self> {
        self.checked_sub(&rhs).ok_or(MathError::Negative)
    }

    /// `self * mul / div`, rounding down.
    fn try_mul_div_floor(self, mul: Self, div: Self) -> MathResult<Self>;
    /// `self * mul / div`, rounding up.
    fn try_mul_div_ceil(self, mul: Self, div: Self) -> MathResult<Self>;
    /// `self * mul / div` with a double-width intermediate, rounding down.
    fn try_widening_mul_div_floor(self, mul: Self, div: Self) -> MathResult<Self>;
    /// `self * factor` for fee factors in `[0, 1)` ranges.
    fn try_mul_f64(self, factor: f64) -> MathResult<Self>;
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
