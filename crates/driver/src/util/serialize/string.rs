use {
    serde::{de, Deserializer, Serializer},
    serde_with::{DeserializeAs, SerializeAs},
    std::marker::PhantomData,
};

/// Serialize and deserialize values using [`std::string::ToString`] and
/// [`std::str::FromStr`].
#[derive(Debug)]
pub struct String;

impl<'de, T> DeserializeAs<'de, T> for String
where
    T: std::str::FromStr,
    <T as std::str::FromStr>::Err: std::error::Error,
{
    fn deserialize_as<D: Deserializer<'de>>(deserializer: D) -> Result<T, D::Error> {
        #[derive(Debug)]
        struct Visitor<T>(PhantomData<T>);

        impl<'de, T> de::Visitor<'de> for Visitor<T>
        where
            T: std::str::FromStr,
            <T as std::str::FromStr>::Err: std::error::Error,
        {
            type Value = T;

            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                write!(formatter, "string")
            }

            fn visit_str<E>(self, s: &str) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                std::str::FromStr::from_str(s)
                    .map_err(|err| de::Error::custom(format!("failed to decode {s:?}: {err:?}")))
            }
        }

        deserializer.deserialize_str(Visitor::<T>(Default::default()))
    }
}

impl<T: std::string::ToString> SerializeAs<T> for String {
    fn serialize_as<S: Serializer>(source: &T, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&source.to_string())
    }
}
