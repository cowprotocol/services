//! CoW Protocol order identifier.

use std::fmt;

/// A 32-byte CoW Protocol order identifier, equal to `hash(intent)`,
/// serialized as a `0x`-prefixed hex string on the wire.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct OrderUid(pub [u8; 32]);

impl fmt::Display for OrderUid {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut bytes = [0u8; 2 + 32 * 2];
        bytes[..2].copy_from_slice(b"0x");
        // Unwrap: the destination length always matches the input.
        const_hex::encode_to_slice(self.0.as_slice(), &mut bytes[2..]).unwrap();
        // Unwrap: hex output is always valid UTF-8.
        f.write_str(std::str::from_utf8(&bytes).unwrap())
    }
}

impl fmt::Debug for OrderUid {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self}")
    }
}

impl serde::Serialize for OrderUid {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_str(self)
    }
}
