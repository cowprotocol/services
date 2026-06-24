//! Domain types for the Solana settlement indexer.

use derive_more::{Display, From, Into};

pub mod channel;
pub mod commitment;
pub mod dead_letter;
pub mod errors;
pub mod events;
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

/// A 32-byte CoW Protocol order identifier, equal to `hash(intent)`.
///
/// This is the value used to derive the order PDA seed (`["settlement",
/// hash(intent), "order"]`). SolFlow custody records and trade-delta
/// accounting carry the same bytes as `order_uid`, and the settlement
/// program's order-lifecycle events expose the same value under the
/// `intent_hash` field name. Per the CoW Solana spec there is exactly one
/// such identifier per order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Display, From, Into)]
#[display("OrderUid({_0:?})")]
pub struct OrderUid(pub [u8; 32]);
