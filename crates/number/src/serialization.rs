use {
    derive_more::{From, Into},
    primitive_types,
    serde::{
        de::{self, Visitor},
        Deserialize,
        Deserializer,
        Serialize,
        Serializer,
    },
    serde_with::SerializeAs,
    std::{
        fmt,
        hash::Hash,
        ops::{Deref, DerefMut},
    },
    uint::FromDecStrErr,
};

/// Serialize [`U256`] as a decimal string a deserialize [`U256`] from a decimal
/// or a hex prefixed with 0x
#[derive(Debug, Clone, Copy, Default, From, Into, Hash, PartialEq, Eq, PartialOrd, Ord)]
pub struct U256(primitive_types::U256);

impl<'de> Deserialize<'de> for U256 {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct U256Visitor;

        impl<'de> Visitor<'de> for U256Visitor {
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
                    Ok(U256(primitive_types::U256::from_str_radix(s, 16).map_err(
                        |err| E::custom(format!("failed to decode {s:?} as hex u256: {err}")),
                    )?))
                } else {
                    Ok(U256(primitive_types::U256::from_dec_str(s).map_err(
                        |err| E::custom(format!("failed to decode {s:?} as decimal u256: {err}")),
                    )?))
                }
            }
        }

        deserializer.deserialize_str(U256Visitor)
    }
}

impl SerializeAs<primitive_types::U256> for U256 {
    fn serialize_as<S: Serializer>(
        source: &primitive_types::U256,
        serializer: S,
    ) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&source.to_string())
    }
}

impl Serialize for U256 {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        U256::serialize_as(&self.0, serializer)
    }
}

impl U256 {
    pub fn zero() -> Self {
        Self(primitive_types::U256::zero())
    }

    pub fn from_dec_str(value: &str) -> Result<Self, FromDecStrErr> {
        Ok(Self(primitive_types::U256::from_dec_str(value)?))
    }
}

impl From<u128> for U256 {
    fn from(value: u128) -> Self {
        Self(primitive_types::U256::from(value))
    }
}

impl From<u64> for U256 {
    fn from(value: u64) -> Self {
        Self(value.into())
    }
}

impl From<u32> for U256 {
    fn from(value: u32) -> Self {
        Self(value.into())
    }
}

impl From<u8> for U256 {
    fn from(value: u8) -> Self {
        Self(value.into())
    }
}

impl From<u16> for U256 {
    fn from(value: u16) -> Self {
        Self(value.into())
    }
}

impl Deref for U256 {
    type Target = primitive_types::U256;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for U256 {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        serde::de::{
            value::{Error as ValueError, StrDeserializer},
            IntoDeserializer,
        },
        std::hash::{DefaultHasher, Hasher},
    };

    #[test]
    fn test_deserialization() {
        let deserializer: StrDeserializer<ValueError> = "0x10".into_deserializer();
        assert_eq!(U256::deserialize(deserializer), Ok(16_u32.into()));

        let deserializer: StrDeserializer<ValueError> = "10".into_deserializer();
        assert_eq!(U256::deserialize(deserializer), Ok(10_u32.into()));
    }

    #[test]
    fn test_deserialization_from_json() {
        let result: U256 = serde_json::from_str(r#""0x10""#).expect("Valid U256");
        assert_eq!(result, 16_u32.into());

        let result: U256 = serde_json::from_str(r#""10""#).expect("Valid U256");
        assert_eq!(result, 10_u32.into());

        assert!(serde_json::from_str::<U256>(r#""10e""#).is_err());
        assert!(serde_json::from_str::<U256>(r#""0xx1""#).is_err());
        assert!(serde_json::from_str::<U256>(r#""0AFF""#).is_err());
    }

    #[test]
    fn test_serialization() {
        let result = U256::from(10_u32);
        assert_eq!(result, 10_u32.into());

        let serialized = serde_json::to_string(&result).expect("Failed to serialize");
        assert_eq!(serialized, "\"10\"");
    }

    #[test]
    fn test_hash_derived_correctly() {
        let value_u256 = U256::from(10_u32);
        let value_primitive_types_u256 = primitive_types::U256::from(10);

        let mut hasher_u256 = DefaultHasher::new();
        let mut hasher_primitive_u256 = DefaultHasher::new();

        value_u256.hash(&mut hasher_u256);
        value_primitive_types_u256.hash(&mut hasher_primitive_u256);

        let hash_u256 = hasher_u256.finish();
        let hash_primitive_u256 = hasher_primitive_u256.finish();

        assert_eq!(hash_u256, hash_primitive_u256, "Hashes should be equal");
    }
}
