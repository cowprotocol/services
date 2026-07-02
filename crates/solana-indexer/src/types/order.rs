//! CoW Protocol order identifier.

use {
    derive_more::{From, Into},
    std::fmt,
};

/// A 32-byte CoW Protocol order identifier, equal to `hash(intent)`.
///
/// This is the value used to derive the order PDA seed (`["settlement",
/// hash(intent), "order"]`). SolFlow custody records and trade-delta
/// accounting carry the same bytes as `order_uid`, and the settlement
/// program's order-lifecycle events expose the same value under the
/// `intent_hash` field name. Per the CoW Solana spec there is exactly one
/// such identifier per order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, From, Into)]
pub(crate) struct OrderUid(pub [u8; 32]);

impl fmt::Display for OrderUid {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "OrderUid(0x")?;
        for byte in self.0 {
            write!(f, "{byte:02x}")?;
        }
        write!(f, ")")
    }
}
