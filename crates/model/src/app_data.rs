use {
    serde::{de, Deserializer, Serializer},
    serde_with::serde::{Deserialize, Serialize},
    std::{
        borrow::Cow,
        fmt::{self, Debug, Formatter},
        str::FromStr,
    },
};

/// A JSON object used to represent app data documents for uploading and
/// retrieving from the API services.
#[derive(Clone, Debug, Default, Deserialize, Serialize, Eq, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AppDataDocument {
    pub full_app_data: String,
}

/// On the contract level orders have 32 bytes of generic data that are freely
/// choosable by the user. On the services level this is a hash of an app data
/// json document, which associates arbitrary information with an order while
/// being signed by the user.
#[derive(Clone, Copy, Default, Eq, Hash, PartialEq)]
pub struct AppDataHash(pub [u8; 32]);

impl AppDataHash {
    pub fn is_zero(&self) -> bool {
        *self == Self::default()
    }
}

impl Debug for AppDataHash {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        write!(f, "0x{}", hex::encode(self.0))
    }
}

impl FromStr for AppDataHash {
    type Err = hex::FromHexError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut bytes = [0u8; 32];
        hex::decode_to_slice(s.strip_prefix("0x").unwrap_or(s), &mut bytes)?;
        Ok(Self(bytes))
    }
}

impl Serialize for AppDataHash {
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

impl<'de> Deserialize<'de> for AppDataHash {
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

impl PartialEq<[u8; 32]> for AppDataHash {
    fn eq(&self, other: &[u8; 32]) -> bool {
        self.0 == *other
    }
}

#[cfg(test)]
mod tests {
    use {super::*, serde_json::json};

    #[test]
    fn works_on_32_byte_string_with_or_without_0x() {
        let with_0x = "0x0ddeb6e4a814908832cc25d11311c514e7efe6af3c9bafeb0d241129cf7f4d83";
        let without_0x = "0ddeb6e4a814908832cc25d11311c514e7efe6af3c9bafeb0d241129cf7f4d83";
        assert!(AppDataHash::from_str(with_0x).is_ok());
        assert!(AppDataHash::from_str(without_0x).is_ok());
        assert_eq!(
            AppDataHash::from_str(with_0x),
            AppDataHash::from_str(without_0x)
        );
    }

    #[test]
    fn invalid_characters() {
        assert_eq!(
            AppDataHash::from_str(
                "xyxyxyxyxyxyxyxyxyxyxyxyxyxyxyxyxyxyxyxyxyxyxyxyxyxyxyxyxyxyxyxy"
            )
            .unwrap_err()
            .to_string(),
            "Invalid character 'x' at position 0"
        );
    }

    #[test]
    fn invalid_length() {
        assert_eq!(
            AppDataHash::from_str("0x00").unwrap_err().to_string(),
            "Invalid string length"
        );
    }

    #[test]
    fn deserialize_app_id() {
        let value = json!("0x0ddeb6e4a814908832cc25d11311c514e7efe6af3c9bafeb0d241129cf7f4d83");
        assert!(AppDataHash::deserialize(value).is_ok());
        assert!(AppDataHash::deserialize(json!("00")).is_err());
        assert!(AppDataHash::deserialize(json!("asdf")).is_err());
        assert!(AppDataHash::deserialize(json!("0x00")).is_err());
    }
}
