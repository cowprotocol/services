//! Serialize and deserialize [`U256`] as a decimal string.

use {
    serde::{de, Deserializer, Serializer},
    serde_with::{DeserializeAs, SerializeAs},
};

#[derive(Debug)]
pub struct Hex;

impl<'de> DeserializeAs<'de, Vec<u8>> for Hex {
    fn deserialize_as<D: Deserializer<'de>>(deserializer: D) -> Result<Vec<u8>, D::Error> {
        struct Visitor;

        impl<'de> de::Visitor<'de> for Visitor {
            type Value = Vec<u8>;

            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                write!(formatter, "a hex-encoded string starting with \"0x\"")
            }

            fn visit_str<E>(self, s: &str) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                if !s.starts_with("0x") {
                    return Err(de::Error::custom(format!(
                        "failed to decode {s:?} as a hex string: missing \"0x\" prefix",
                    )));
                }
                hex::decode(&s[2..]).map_err(|err| {
                    de::Error::custom(format!("failed to decode {s:?} as a hex string: {err}",))
                })
            }
        }

        deserializer.deserialize_str(Visitor)
    }
}

impl SerializeAs<Vec<u8>> for Hex {
    fn serialize_as<S: Serializer>(source: &Vec<u8>, serializer: S) -> Result<S::Ok, S::Error> {
        let hex = hex::encode(source);
        serializer.serialize_str(&format!("0x{hex}"))
    }
}
