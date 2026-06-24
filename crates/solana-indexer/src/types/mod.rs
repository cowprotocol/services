//! Domain types for the Solana settlement indexer.

use derive_more::{Display, From, Into};

pub mod channel;
pub mod commitment;
pub mod dead_letter;
pub mod errors;
pub mod events;
pub mod metrics;
pub mod recovery;
pub mod tx;
pub mod wire;

/// A Solana ledger slot.
///
/// This is the type used throughout the indexer to represent a Solana ledger
/// slot.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Display, From, Into)]
#[display("Slot({})", _0)]
pub struct Slot(pub u64);

pub use solana_sdk::signature::Signature;
