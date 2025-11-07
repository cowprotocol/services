//! This crate contains the data transfer object that solver engines use to
//! communicate with the driver.

pub mod auction;
pub mod notification;
pub mod solution;

mod serialize {
    use {
        serde::{Deserializer, Serializer, de},
        serde_with::{DeserializeAs, SerializeAs},
    };

    /// Serialize and deserialize binary data as a hexadecimal string.
    #[derive(Debug)]
    pub struct Hex;

    impl<'de> DeserializeAs<'de, Vec<u8>> for Hex {
        fn deserialize_as<D: Deserializer<'de>>(deserializer: D) -> Result<Vec<u8>, D::Error> {
            struct Visitor;

            impl de::Visitor<'_> for Visitor {
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
                    const_hex::decode(&s[2..]).map_err(|err| {
                        de::Error::custom(format!("failed to decode {s:?} as a hex string: {err}",))
                    })
                }
            }

            deserializer.deserialize_str(Visitor)
        }
    }

    impl SerializeAs<Vec<u8>> for Hex {
        fn serialize_as<S: Serializer>(source: &Vec<u8>, serializer: S) -> Result<S::Ok, S::Error> {
            serializer.serialize_str(&bytes_to_hex_string(source.as_ref()))
        }
    }

    impl<'de, const N: usize> DeserializeAs<'de, [u8; N]> for Hex {
        fn deserialize_as<D: Deserializer<'de>>(deserializer: D) -> Result<[u8; N], D::Error> {
            struct Visitor<const N: usize> {
                result: [u8; N],
            }

            impl<const N: usize> de::Visitor<'_> for Visitor<N> {
                type Value = [u8; N];

                fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                    write!(
                        formatter,
                        "a hex-encoded string starting with \"0x\" containing {N} bytes",
                    )
                }

                fn visit_str<E>(mut self, s: &str) -> Result<Self::Value, E>
                where
                    E: de::Error,
                {
                    if !s.starts_with("0x") {
                        return Err(de::Error::custom(format!(
                            "failed to decode {s:?} as a hex string: missing \"0x\" prefix",
                        )));
                    }
                    let decoded = const_hex::decode(&s[2..]).map_err(|err| {
                        de::Error::custom(format!("failed to decode {s:?} as a hex string: {err}",))
                    })?;
                    if decoded.len() != N {
                        return Err(de::Error::custom(format!(
                            "failed to decode {s:?} as a hex string: expected {N} bytes, got {}",
                            decoded.len()
                        )));
                    }
                    self.result.copy_from_slice(&decoded);
                    Ok(self.result)
                }
            }

            deserializer.deserialize_str(Visitor { result: [0; N] })
        }
    }

    impl<const N: usize> SerializeAs<[u8; N]> for Hex {
        fn serialize_as<S: Serializer>(source: &[u8; N], serializer: S) -> Result<S::Ok, S::Error> {
            serializer.serialize_str(&bytes_to_hex_string(source))
        }
    }

    fn bytes_to_hex_string(bytes: &[u8]) -> String {
        let mut v = vec![0u8; 2 + bytes.len() * 2];
        v[0] = b'0';
        v[1] = b'x';
        // Unwrap because only possible error is vector wrong size which cannot happen.
        const_hex::encode_to_slice(bytes, &mut v[2..]).unwrap();
        // Unwrap because encoded data is always valid utf8.
        String::from_utf8(v).unwrap()
    }
}
