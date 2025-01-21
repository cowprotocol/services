use {
    crate::{app_data_hash::hash_full_app_data, AppDataHash, Hooks},
    anyhow::{anyhow, Context, Result},
    primitive_types::{H160, U256},
    serde::{de, Deserialize, Deserializer, Serialize, Serializer},
    std::{fmt, fmt::Display},
};

/// The minimum valid empty app data JSON string.
pub const EMPTY: &str = "{}";

#[derive(Clone, Debug, PartialEq)]
pub struct ValidatedAppData {
    pub hash: AppDataHash,
    pub document: String,
    pub protocol: ProtocolAppData,
}

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq)]
#[cfg_attr(any(test, feature = "test_helpers"), derive(Serialize))]
#[serde(rename_all = "camelCase")]
pub struct ProtocolAppData {
    #[serde(default)]
    pub hooks: Hooks,
    pub signer: Option<H160>,
    pub replaced_order: Option<ReplacedOrder>,
    pub partner_fee: Option<PartnerFee>,
    pub flashloan: Option<Flashloan>,
}

/// Contains information to hint at how a solver could make
/// use of flashloans to settle the associated order.
/// Since using flashloans introduces a bunch of complexities
/// all these hints are not binding for the solver.
#[derive(Clone, Debug, Default, Serialize, Deserialize, Eq, PartialEq)]
pub struct Flashloan {
    /// Which contract to request the flashloan from.
    pub lender: Option<H160>,
    /// Who should receive the borrowed tokens. If this is not
    /// set the order owner will get the tokens.
    pub borrower: Option<H160>,
    /// Which token to flashloan.
    pub token: H160,
    /// How much of the token to flashloan.
    pub amount: U256,
}

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq)]
#[cfg_attr(any(test, feature = "test_helpers"), derive(Serialize))]
pub struct ReplacedOrder {
    pub uid: OrderUid,
}

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq)]
#[cfg_attr(any(test, feature = "test_helpers"), derive(Serialize))]
pub struct PartnerFee {
    pub bps: u64,
    pub recipient: H160,
}

#[derive(Clone)]
pub struct Validator {
    size_limit: usize,
}

#[cfg(any(test, feature = "test_helpers"))]
impl Default for Validator {
    fn default() -> Self {
        Self { size_limit: 8192 }
    }
}

impl Validator {
    pub fn new(size_limit: usize) -> Self {
        Self { size_limit }
    }

    pub fn size_limit(&self) -> usize {
        self.size_limit
    }

    pub fn validate(&self, full_app_data: &[u8]) -> Result<ValidatedAppData> {
        if full_app_data.len() > self.size_limit {
            return Err(anyhow!(
                "app data has byte size {} which is larger than limit {}",
                full_app_data.len(),
                self.size_limit
            ));
        }

        let document = String::from_utf8(full_app_data.to_vec())?;
        let root = serde_json::from_str::<Root>(&document).context("invalid app data json")?;
        let protocol = root
            .metadata
            .or_else(|| root.backend.map(ProtocolAppData::from))
            // If the key doesn't exist, default. Makes life easier for API
            // consumers, who don't care about protocol app data.
            .unwrap_or_default();

        Ok(ValidatedAppData {
            hash: AppDataHash(hash_full_app_data(full_app_data)),
            document,
            protocol,
        })
    }
}

/// The root app data JSON object.
///
/// App data JSON is organised in an object of the form
///
/// ```text
/// {
///     "metadata": {}
/// }
/// ```
///
/// Where the protocol-relevant app-data fields appear in the `metadata` object
/// along side other valid metadata fields. For example:
///
/// ```text
/// {
///     "version": "0.9.0",
///     "appCode": "CoW Swap",
///     "environment": "barn",
///     "metadata": {
///         "quote": {
///             "slippageBps": "50"
///         },
///         "hooks": {
///             "pre": [
///                 {
///                     "target": "0x0000000000000000000000000000000000000000",
///                     "callData": "0x",
///                     "gasLimit": "21000"
///                 }
///             ]
///         }
///     }
/// }
/// ```
///
/// For more detailed information on the schema, see:
/// <https://github.com/cowprotocol/app-data>.
#[derive(Clone, Deserialize)]
#[cfg_attr(any(test, feature = "test_helpers"), derive(Serialize))]
pub struct Root {
    metadata: Option<ProtocolAppData>,
    /// DEPRECATED. The `backend` field was originally specified to contain all
    /// protocol-specific app data (such as hooks). However, after releasing
    /// hooks, we decided to move the fields to the existing `metadata` field.
    /// However, in order to not break existing integrations, we allow using the
    /// `backend` field for specifying hooks.
    backend: Option<BackendAppData>,
}

impl Root {
    pub fn new(metadata: Option<ProtocolAppData>) -> Self {
        Self {
            metadata,
            backend: None,
        }
    }
}

// uid as 56 bytes: 32 for orderDigest, 20 for ownerAddress and 4 for validTo
#[derive(Clone, Copy, Eq, Hash, PartialEq, PartialOrd, Ord)]
pub struct OrderUid(pub [u8; 56]);

impl Display for OrderUid {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut bytes = [0u8; 2 + 56 * 2];
        bytes[..2].copy_from_slice(b"0x");
        // Unwrap because the length is always correct.
        hex::encode_to_slice(self.0.as_slice(), &mut bytes[2..]).unwrap();
        // Unwrap because the string is always valid utf8.
        let str = std::str::from_utf8(&bytes).unwrap();
        f.write_str(str)
    }
}

impl fmt::Debug for OrderUid {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self}")
    }
}

impl Default for OrderUid {
    fn default() -> Self {
        Self([0u8; 56])
    }
}

impl Serialize for OrderUid {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(self.to_string().as_str())
    }
}

impl<'de> Deserialize<'de> for OrderUid {
    fn deserialize<D>(deserializer: D) -> Result<OrderUid, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct Visitor {}
        impl de::Visitor<'_> for Visitor {
            type Value = OrderUid;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                write!(formatter, "an uid with orderDigest_owner_validTo")
            }

            fn visit_str<E>(self, s: &str) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                let s = s.strip_prefix("0x").ok_or_else(|| {
                    de::Error::custom(format!(
                        "{s:?} can't be decoded as hex uid because it does not start with '0x'"
                    ))
                })?;
                let mut value = [0u8; 56];
                hex::decode_to_slice(s, value.as_mut()).map_err(|err| {
                    de::Error::custom(format!("failed to decode {s:?} as hex uid: {err}"))
                })?;
                Ok(OrderUid(value))
            }
        }

        deserializer.deserialize_str(Visitor {})
    }
}

/// The legacy `backend` app data object.
#[derive(Clone, Debug, Default, Deserialize)]
#[cfg_attr(any(test, feature = "test_helpers"), derive(Serialize))]
struct BackendAppData {
    #[serde(default)]
    pub hooks: Hooks,
}

impl From<BackendAppData> for ProtocolAppData {
    fn from(value: BackendAppData) -> Self {
        Self {
            hooks: value.hooks,
            signer: None,
            replaced_order: None,
            partner_fee: None,
            flashloan: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use {super::*, crate::Hook, ethcontract::H160};

    macro_rules! assert_app_data {
        ($s:expr, $e:expr $(,)?) => {{
            let s = $s;
            let a = Validator::default().validate(s.as_ref()).unwrap();
            assert_eq!(a.protocol, $e);
        }};
    }

    #[test]
    fn empty_is_valid() {
        assert_app_data!(EMPTY, ProtocolAppData::default());
    }

    #[test]
    fn examples() {
        assert_app_data!(
            r#"
                {
                    "appCode": "CoW Swap",
                    "environment": "production",
                    "version": "0.9.0"
                }
            "#,
            ProtocolAppData::default(),
        );

        assert_app_data!(
            r#"
                {
                    "appCode": "CoW Swap",
                    "environment": "production",
                    "metadata": {
                        "quote": {
                            "slippageBips": "50"
                        },
                        "orderClass": {
                            "orderClass": "market"
                        }
                    },
                    "version": "0.9.0"
                }
            "#,
            ProtocolAppData::default(),
        );

        assert_app_data!(
            r#"
                {
                    "appCode": "CoW Swap",
                    "environment": "production",
                    "metadata": {
                        "quote": {
                            "slippageBips": "50"
                        },
                        "orderClass": {
                            "orderClass": "market"
                        },
                        "hooks": {
                            "pre": [
                                {
                                    "target": "0x0000000000000000000000000000000000000000",
                                    "callData": "0x",
                                    "gasLimit": "0"
                                }
                            ],
                            "post": [
                                {
                                    "target": "0x0101010101010101010101010101010101010101",
                                    "callData": "0x01",
                                    "gasLimit": "1"
                                },
                                {
                                    "target": "0x0202020202020202020202020202020202020202",
                                    "callData": "0x0202",
                                    "gasLimit": "2"
                                }
                            ]
                        }
                    },
                    "version": "0.9.0"
                }
            "#,
            ProtocolAppData {
                hooks: Hooks {
                    pre: vec![Hook {
                        target: H160([0; 20]),
                        call_data: vec![],
                        gas_limit: 0,
                    }],
                    post: vec![
                        Hook {
                            target: H160([1; 20]),
                            call_data: vec![1],
                            gas_limit: 1
                        },
                        Hook {
                            target: H160([2; 20]),
                            call_data: vec![2, 2],
                            gas_limit: 2,
                        },
                    ],
                },
                ..Default::default()
            },
        );

        assert_app_data!(
            r#"
                {
                    "appCode": "CoW Swap",
                    "environment": "production",
                    "metadata": {
                        "signer": "0x4242424242424242424242424242424242424242"
                    },
                    "version": "0.9.0"
                }
            "#,
            ProtocolAppData {
                signer: Some(H160([0x42; 20])),
                ..Default::default()
            },
        );
    }

    #[test]
    fn legacy() {
        assert_app_data!(
            r#"
                {
                    "backend": {
                        "hooks": {
                            "pre": [
                                {
                                    "target": "0x0000000000000000000000000000000000000000",
                                    "callData": "0x",
                                    "gasLimit": "0"
                                }
                            ],
                            "post": [
                                {
                                    "target": "0x0101010101010101010101010101010101010101",
                                    "callData": "0x01",
                                    "gasLimit": "1"
                                },
                                {
                                    "target": "0x0202020202020202020202020202020202020202",
                                    "callData": "0x0202",
                                    "gasLimit": "2"
                                }
                            ]
                        }
                    }
                }
            "#,
            ProtocolAppData {
                hooks: Hooks {
                    pre: vec![Hook {
                        target: H160([0; 20]),
                        call_data: vec![],
                        gas_limit: 0,
                    }],
                    post: vec![
                        Hook {
                            target: H160([1; 20]),
                            call_data: vec![1],
                            gas_limit: 1
                        },
                        Hook {
                            target: H160([2; 20]),
                            call_data: vec![2, 2],
                            gas_limit: 2,
                        },
                    ],
                },
                ..Default::default()
            },
        );

        // Note that if `metadata` is specified, then the `backend` field is
        // ignored.
        assert_app_data!(
            r#"
                {
                    "metadata": {},
                    "backend": {
                        "hooks": {
                            "pre": [
                                {
                                    "target": "0x0000000000000000000000000000000000000000",
                                    "callData": "0x",
                                    "gasLimit": "0"
                                }
                            ]
                        }
                    }
                }
            "#,
            ProtocolAppData::default(),
        );
    }

    #[test]
    fn misc() {
        let mut validator = Validator::default();

        let not_json = "hello world".as_bytes();
        let err = validator.validate(not_json).unwrap_err();
        dbg!(err);

        let not_object = "[]".as_bytes();
        let err = validator.validate(not_object).unwrap_err();
        dbg!(err);

        let object = "{}".as_bytes();
        let validated = validator.validate(object).unwrap();
        dbg!(validated.hash);

        let ok_no_metadata = r#"{"hello":"world"}"#.as_bytes();
        validator.validate(ok_no_metadata).unwrap();

        let bad_metadata = r#"{"hello":"world","metadata":[1]}"#.as_bytes();
        let err = validator.validate(bad_metadata).unwrap_err();
        dbg!(err);

        let ok_metadata = r#"{"hello":"world","metadata":{}}"#.as_bytes();
        validator.validate(ok_metadata).unwrap();

        validator.size_limit = 1;
        let size_limit = r#"{"hello":"world"}"#.as_bytes();
        let err = validator.validate(size_limit).unwrap_err();
        dbg!(err);
    }
}
