use {
    primitive_types::U256,
    serde::{de, Deserializer, Serializer},
    serde_with::{DeserializeAs, SerializeAs},
    std::fmt,
};

pub struct HexOrDecimalU256;

impl<'de> DeserializeAs<'de, U256> for HexOrDecimalU256 {
    fn deserialize_as<D>(deserializer: D) -> Result<U256, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserialize(deserializer)
    }
}

impl SerializeAs<U256> for HexOrDecimalU256 {
    fn serialize_as<S>(source: &U256, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serialize(source, serializer)
    }
}

pub fn serialize<S>(value: &U256, serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    serializer.serialize_str(&value.to_string())
}

pub fn deserialize<'de, D>(deserializer: D) -> Result<U256, D::Error>
where
    D: Deserializer<'de>,
{
    struct Visitor {}
    impl de::Visitor<'_> for Visitor {
        type Value = U256;

        fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
            write!(
                formatter,
                "a u256 encoded either as 0x hex prefixed or decimal encoded string"
            )
        }

        fn visit_str<E>(self, s: &str) -> Result<Self::Value, E>
        where
            E: de::Error,
        {
            if s.trim().starts_with("0x") {
                U256::from_str_radix(s, 16).map_err(|err| {
                    de::Error::custom(format!("failed to decode {s:?} as hex u256: {err}"))
                })
            } else {
                U256::from_dec_str(s).map_err(|err| {
                    de::Error::custom(format!("failed to decode {s:?} as decimal u256: {err}"))
                })
            }
        }
    }

    deserializer.deserialize_str(Visitor {})
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        serde::de::{
            value::{Error as ValueError, StrDeserializer},
            IntoDeserializer,
        },
    };

    #[test]
    fn test_deserialization() {
        let deserializer: StrDeserializer<ValueError> = "0x10".into_deserializer();
        assert_eq!(deserialize(deserializer), Ok(16.into()));

        let deserializer: StrDeserializer<ValueError> = "10".into_deserializer();
        assert_eq!(deserialize(deserializer), Ok(10.into()));
    }
}
