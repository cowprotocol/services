use primitive_types::H160;
use serde::{de, Deserializer, Serializer};
use std::fmt;

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
