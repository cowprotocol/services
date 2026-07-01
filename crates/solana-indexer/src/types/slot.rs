//! Solana ledger slot.

use derive_more::{Display, From, Into};

/// A Solana ledger slot.
///
/// This is the type used throughout the indexer to represent a Solana ledger
/// slot.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Display, From, Into)]
#[display("Slot({})", _0)]
pub(crate) struct Slot(pub u64);
