//! CoW Protocol order identifier.

use std::{fmt, str::FromStr};

/// A 32-byte CoW Protocol order identifier, equal to `hash(intent)`, serialized
/// as a `0x`-prefixed hex string on the wire.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct OrderUid(pub [u8; 32]);

impl fmt::Display for OrderUid {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut buffer = const_hex::Buffer::<32, true>::new();
        f.write_str(buffer.format(&self.0))
    }
}

impl fmt::Debug for OrderUid {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self}")
    }
}

impl FromStr for OrderUid {
    type Err = const_hex::FromHexError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut bytes = [0u8; 32];
        const_hex::decode_to_slice(s.strip_prefix("0x").unwrap_or(s), &mut bytes)?;
        Ok(Self(bytes))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn order_uid_roundtrip() {
        let uid = OrderUid([0xab; 32]);
        let text = uid.to_string();
        assert_eq!(text, format!("0x{}", "ab".repeat(32)));
        assert_eq!(OrderUid::from_str(&text).unwrap(), uid);
    }
}
