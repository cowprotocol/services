use primitive_types::H160;
use serde::{de, Deserializer, Serializer};
use serde_with::{DeserializeAs, SerializeAs};
use std::fmt;

pub struct HexadecimalH160;

impl<'de> DeserializeAs<'de, H160> for HexadecimalH160 {
    fn deserialize_as<D>(deserializer: D) -> Result<H160, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserialize(deserializer)
    }
}

impl<'de> SerializeAs<H160> for HexadecimalH160 {
    fn serialize_as<S>(source: &H160, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serialize(source, serializer)
    }
}

pub fn serialize<S>(value: &H160, serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    let mut bytes = [0u8; 2 + 20 * 2];
    bytes[..2].copy_from_slice(b"0x");
    // Can only fail if the buffer size does not match but we know it is correct.
    hex::encode_to_slice(value, &mut bytes[2..]).unwrap();
    // Hex encoding is always valid utf8.
    let s = std::str::from_utf8(&bytes).unwrap();
    serializer.serialize_str(s)
}

pub fn deserialize<'de, D>(deserializer: D) -> Result<H160, D::Error>
where
    D: Deserializer<'de>,
{
    struct Visitor {}
    impl<'de> de::Visitor<'de> for Visitor {
        type Value = H160;

        fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
            write!(formatter, "an ethereum address as a hex encoded string")
        }

        fn visit_str<E>(self, s: &str) -> Result<Self::Value, E>
        where
            E: de::Error,
        {
            let s = s.strip_prefix("0x").ok_or_else(|| {
                de::Error::custom(format!(
                    "{:?} can't be decoded as hex h160 because it does not start with '0x'",
                    s
                ))
            })?;
            let mut value = H160::zero();
            hex::decode_to_slice(s, value.as_mut()).map_err(|err| {
                de::Error::custom(format!("failed to decode {:?} as hex h160: {}", s, err))
            })?;
            Ok(value)
        }
    }

    deserializer.deserialize_str(Visitor {})
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;

    #[test]
    fn works_on_20_byte_string() {
        let value = Value::String("0x9008D19f58AAbD9eD0D60971565AA8510560ab41".to_string());
        assert!(deserialize(value).is_ok());
    }

    #[test]
    fn does_not_start_with_0x() {
        let value = Value::String("00".to_string());
        assert!(deserialize(value).is_err());
    }

    #[test]
    fn invalid_characters() {
        let value = Value::String("asdf".to_string());
        assert!(deserialize(value).is_err());
    }

    #[test]
    fn invalid_length() {
        let value = Value::String("0x00".to_string());
        assert!(deserialize(value).is_err());
    }
}
