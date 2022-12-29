use serde::{de, Deserialize, Deserializer, Serializer};
use serde_with::{DeserializeAs, SerializeAs};
use std::borrow::Cow;

/// Serialize and deserialize binary data as a hexadecimal string.
#[derive(Debug)]
pub struct Hex;

impl<'de> DeserializeAs<'de, Vec<u8>> for Hex {
    fn deserialize_as<D: Deserializer<'de>>(deserializer: D) -> Result<Vec<u8>, D::Error> {
        let s = Cow::<str>::deserialize(deserializer)?;
        let s = s
            .strip_prefix("0x")
            .ok_or_else(
                || format!("failed to decode {s:?} as a hex string: missing \"0x\" prefix",),
            )
            .map_err(de::Error::custom)?;
        hex::decode(s).map_err(|err| {
            de::Error::custom(format!("failed to decode {s:?} as a hex string: {err}",))
        })
    }
}

impl SerializeAs<Vec<u8>> for Hex {
    fn serialize_as<S: Serializer>(source: &Vec<u8>, serializer: S) -> Result<S::Ok, S::Error> {
        let hex = hex::encode(source);
        serializer.serialize_str(&format!("0x{hex}"))
    }
}

impl<'de, const N: usize> DeserializeAs<'de, [u8; N]> for Hex {
    fn deserialize_as<D: Deserializer<'de>>(deserializer: D) -> Result<[u8; N], D::Error> {
        let s = Cow::<str>::deserialize(deserializer)?;
        let s = s
            .strip_prefix("0x")
            .ok_or_else(
                || format!("failed to decode {s:?} as a hex string: missing \"0x\" prefix",),
            )
            .map_err(de::Error::custom)?;

        let mut buffer = [0; N];
        hex::decode_to_slice(s, &mut buffer).map_err(|err| {
            de::Error::custom(format!("failed to decode {s:?} as a hex string: {err}",))
        })?;
        Ok(buffer)
    }
}

impl<const N: usize> SerializeAs<[u8; N]> for Hex {
    fn serialize_as<S: Serializer>(source: &[u8; N], serializer: S) -> Result<S::Ok, S::Error> {
        let hex = hex::encode(source);
        serializer.serialize_str(&format!("0x{hex}"))
    }
}
