use serde::{de, Deserializer, Serializer};
use serde_with::serde::{Deserialize, Serialize};
use std::{
    borrow::Cow,
    fmt::{self, Debug, Formatter},
    str::FromStr,
};

/// This allows arbitrary user data to be associated with an order. This type holds the
/// hash of the data, while the data itself is uploaded to IPFS. The hash is signed along with the
/// order.
#[derive(Clone, Copy, Default, Eq, Hash, PartialEq)]
pub struct AppId(pub [u8; 32]);

impl Debug for AppId {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        write!(f, "0x{}", hex::encode(self.0))
    }
}

impl FromStr for AppId {
    type Err = hex::FromHexError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut bytes = [0u8; 32];
        hex::decode_to_slice(s.strip_prefix("0x").unwrap_or(s), &mut bytes)?;
        Ok(Self(bytes))
    }
}

impl Serialize for AppId {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut bytes = [0u8; 2 + 32 * 2];
        bytes[..2].copy_from_slice(b"0x");
        // Can only fail if the buffer size does not match but we know it is correct.
        hex::encode_to_slice(self.0, &mut bytes[2..]).unwrap();
        // Hex encoding is always valid utf8.
        let s = std::str::from_utf8(&bytes).unwrap();
        serializer.serialize_str(s)
    }
}

impl<'de> Deserialize<'de> for AppId {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = Cow::<str>::deserialize(deserializer)?;
        let value = s.parse().map_err(|err| {
            de::Error::custom(format!(
                "failed to decode {s:?} as hex appdata 32 bytes: {err}"
            ))
        })?;
        Ok(value)
    }
}

impl PartialEq<[u8; 32]> for AppId {
    fn eq(&self, other: &[u8; 32]) -> bool {
        self.0 == *other
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn works_on_32_byte_string_with_or_without_0x() {
        let with_0x = "0x0ddeb6e4a814908832cc25d11311c514e7efe6af3c9bafeb0d241129cf7f4d83";
        let without_0x = "0ddeb6e4a814908832cc25d11311c514e7efe6af3c9bafeb0d241129cf7f4d83";
        assert!(AppId::from_str(with_0x).is_ok());
        assert!(AppId::from_str(without_0x).is_ok());
        assert_eq!(AppId::from_str(with_0x), AppId::from_str(without_0x));
    }

    #[test]
    fn invalid_characters() {
        assert_eq!(
            AppId::from_str("xyxyxyxyxyxyxyxyxyxyxyxyxyxyxyxyxyxyxyxyxyxyxyxyxyxyxyxyxyxyxyxy")
                .unwrap_err()
                .to_string(),
            "Invalid character 'x' at position 0"
        );
    }

    #[test]
    fn invalid_length() {
        assert_eq!(
            AppId::from_str("0x00").unwrap_err().to_string(),
            "Invalid string length"
        );
    }

    #[test]
    fn deserialize_app_id() {
        let value = json!("0x0ddeb6e4a814908832cc25d11311c514e7efe6af3c9bafeb0d241129cf7f4d83");
        assert!(AppId::deserialize(value).is_ok());
        assert!(AppId::deserialize(json!("00")).is_err());
        assert!(AppId::deserialize(json!("asdf")).is_err());
        assert!(AppId::deserialize(json!("0x00")).is_err());
    }
}
