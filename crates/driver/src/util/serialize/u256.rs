use {
    crate::domain::eth,
    serde::{Deserializer, Serializer, de},
    serde_with::{DeserializeAs, SerializeAs},
};

/// Serialize and deserialize [`eth::U256`] as a decimal string.
#[derive(Debug)]
pub struct U256;

impl<'de> DeserializeAs<'de, eth::U256> for U256 {
    fn deserialize_as<D: Deserializer<'de>>(deserializer: D) -> Result<eth::U256, D::Error> {
        struct Visitor;

        impl de::Visitor<'_> for Visitor {
            type Value = eth::U256;

            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                write!(formatter, "a 256-bit decimal string")
            }

            fn visit_str<E>(self, s: &str) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                eth::U256::from_dec_str(s).map_err(|err| {
                    de::Error::custom(format!("failed to decode {s:?} as a 256-bit number: {err}"))
                })
            }
        }

        deserializer.deserialize_str(Visitor)
    }
}

impl SerializeAs<eth::U256> for U256 {
    fn serialize_as<S: Serializer>(source: &eth::U256, serializer: S) -> Result<S::Ok, S::Error> {
        // `primitive_types::U256::to_string()` is so slow that
        // it's still faster to first convert to alloy's U256
        // and convert that to string...
        let mut buf = [0u8; 32];
        source.to_big_endian(&mut buf);
        let source = alloy::primitives::U256::from_be_bytes(buf);
        serializer.serialize_str(&source.to_string())
    }
}
