//! CoW Protocol order identifier.

use derive_more::{Display, From, Into};

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
pub(crate) struct OrderUid(pub [u8; 32]);
