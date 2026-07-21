//! CoW Protocol order identifier.

use std::fmt;

/// A 32-byte CoW Protocol order identifier, equal to `hash(intent)`. The
/// same bytes the indexer and the settlement program's order-lifecycle
/// events carry, serialized as hex on the wire.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct OrderUid(pub [u8; 32]);

impl fmt::Display for OrderUid {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "0x")?;
        for byte in self.0 {
            write!(f, "{byte:02x}")?;
        }
        Ok(())
    }
}

impl serde::Serialize for OrderUid {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_str(&self)
    }
}
