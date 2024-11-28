//! App data refers to extra information that is associated with orders. This
//! information is not validated by the contract but it is used by other parts
//! of the system. For example, a user could specify that they want their order
//! to be COW only, which is something only the backend understands. Or what
//! their intended slippage when creating the order with the frontend was, which
//! adjusts the signed prices.
//!
//! On the smart contract level app data is freely choosable 32 bytes of signed
//! order data. This isn't enough space for some purposes so we interpret those
//! bytes as a hash of the full app data of arbitrary length. The full app data
//! is thus signed by the user when they sign the order.
//!
//! This crate specifies how the hash is calculated. It takes the keccak256 hash
//! of the input bytes. Additionally, it provides a canonical way to calculate
//! an IPFS CID from the hash. This allows full app data to be uploaded to IPFS.
//!
//! Note that not all app data hashes were created this way. As of 2023-05-25 we
//! are planning to move to the scheme implemented by this crate but orders have
//! been created with arbitrary app data hashes until now. See [this issue][0]
//! for more information.
//!
//! [0]: https://github.com/cowprotocol/services/issues/1465

use {
    serde::{de, Deserializer, Serializer},
    serde_with::serde::{Deserialize, Serialize},
    std::{
        borrow::Cow,
        fmt::{self, Debug, Formatter},
        str::FromStr,
    },
    tiny_keccak::{Hasher, Keccak},
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

/// Hash full app data to get the bytes expected to be set as the contract level
/// app data.
pub fn hash_full_app_data(app_data: &[u8]) -> [u8; 32] {
    let mut hasher = Keccak::v256();
    hasher.update(app_data);
    let mut hash = [0u8; 32];
    hasher.finalize(&mut hash);
    hash
}

/// Create an IPFS CIDv1 from a hash created by `hash_full_app_data`.
///
/// The return value is the raw bytes of the CID. It is not multibase encoded.
pub fn create_ipfs_cid(app_data_hash: &[u8; 32]) -> [u8; 36] {
    let mut cid = [0u8; 4 + 32];
    cid[0] = 1; // cid version
    cid[1] = 0x55; // raw codec
    cid[2] = 0x1b; // keccak hash algorithm
    cid[3] = 32; // keccak hash length
    cid[4..].copy_from_slice(app_data_hash);
    cid
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

    // Alternative way of calculating the expected values:
    // cat appdata | ipfs block put --mhtype keccak-256
    // -> bafkrwiek6tumtfzvo6yivqq5c7jtdkw6q3ar5pgfcjdujvrbzkbwl3eueq
    // ipfs cid format -b base16
    // bafkrwiek6tumtfzvo6yivqq5c7jtdkw6q3ar5pgfcjdujvrbzkbwl3eueq
    // -> f01551b208af4e8c9973577b08ac21d17d331aade86c11ebcc5124744d621ca8365ec9424
    // Remove the f prefix and you have the same CID.
    // Or check out the cid explorer:
    // - https://cid.ipfs.tech/#f01551b208af4e8c9973577b08ac21d17d331aade86c11ebcc5124744d621ca8365ec9424
    // - https://cid.ipfs.tech/#bafkrwiek6tumtfzvo6yivqq5c7jtdkw6q3ar5pgfcjdujvrbzkbwl3eueq
    #[test]
    fn known_good() {
        let full_app_data = r#"{"appCode":"CoW Swap","environment":"production","metadata":{"quote":{"slippageBips":"50","version":"0.2.0"},"orderClass":{"orderClass":"market","version":"0.1.0"}},"version":"0.6.0"}"#;
        let expected_hash =
            hex_literal::hex!("8af4e8c9973577b08ac21d17d331aade86c11ebcc5124744d621ca8365ec9424");
        let expected_cid = hex_literal::hex!(
            "01551b208af4e8c9973577b08ac21d17d331aade86c11ebcc5124744d621ca8365ec9424"
        );
        let hash = hash_full_app_data(full_app_data.as_bytes());
        let cid = create_ipfs_cid(&hash);
        assert_eq!(hash, expected_hash);
        assert_eq!(cid, expected_cid);
    }
}
