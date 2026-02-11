use {
    crate::{AppDataHash, Hooks, app_data_hash::hash_full_app_data},
    alloy::primitives::{Address, U256},
    anyhow::{Context, Result, anyhow},
    bytes_hex::BytesHex,
    moka::sync::Cache,
    number::serialization::HexOrDecimalU256,
    serde::{Deserialize, Deserializer, Serialize, Serializer, de},
    serde_with::serde_as,
    std::{
        fmt::{self, Display},
        slice::Iter,
    },
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
    pub signer: Option<Address>,
    pub replaced_order: Option<ReplacedOrder>,
    #[serde(default)]
    pub partner_fee: PartnerFees,
    pub flashloan: Option<Flashloan>,
    #[serde(default)]
    pub wrappers: Vec<WrapperCall>,
}

/// Contains information to hint at how a solver could make
/// use of flashloans to settle the associated order.
/// Since using flashloans introduces a bunch of complexities
/// all these hints are not binding for the solver.
#[serde_as]
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq)]
#[cfg_attr(any(test, feature = "test_helpers"), derive(Serialize))]
#[serde(rename_all = "camelCase")]
pub struct Flashloan {
    /// Which contract to request the flashloan from.
    pub liquidity_provider: Address,
    /// Which helper contract should be used to request
    /// the flashloan with.
    pub protocol_adapter: Address,
    /// Who should receive the borrowed tokens.
    pub receiver: Address,
    /// Which token to flashloan.
    pub token: Address,
    /// How much of the token to flashloan.
    #[serde_as(as = "HexOrDecimalU256")]
    pub amount: U256,
}

/// Contains information about wrapper contracts
#[serde_as]
#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[cfg_attr(any(test, feature = "test_helpers"), derive(Serialize))]
#[serde(rename_all = "camelCase")]
pub struct WrapperCall {
    /// The address of the wrapper contract.
    pub address: Address,
    /// Additional calldata to be passed to the wrapper contract.
    #[serde_as(as = "BytesHex")]
    pub data: Vec<u8>,
    /// Declares whether this wrapper (and its data) needs to be included
    /// unmodified in a solution containing this order.
    #[serde(default)]
    pub is_omittable: bool,
}

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq)]
#[cfg_attr(any(test, feature = "test_helpers"), derive(Serialize))]
pub struct ReplacedOrder {
    pub uid: OrderUid,
}

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq)]
#[cfg_attr(any(test, feature = "test_helpers"), derive(Serialize))]
pub struct PartnerFee {
    #[serde(flatten)]
    pub policy: FeePolicy,
    pub recipient: Address,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum FeePolicy {
    /// Fees should be captured from the difference between execution price
    /// and the orders' limit price (i.e. improvement over the price signed
    /// by the user).
    Surplus {
        /// How many bps of surplus should be captured as fees.
        bps: u64,
        /// How many bps of the total volume may be captured at most. Under some
        /// conditions there can be a lot of surplus so to not charge egrigious
        /// amounts there is a cap. Note that there is also a cap enforced by
        /// the protocol so effectively the partner can only lower the
        /// limit here.
        max_volume_bps: u64,
    },
    /// Fees should be captured from the difference between execution price
    /// and the price of the order's reference quote (i.e. improvement over the
    /// promised price).
    PriceImprovement {
        /// How many bps of surplus should be captured as fees.
        bps: u64,
        /// How many bps of the total volume may be captured at most. Under some
        /// conditions there can be a lot of surplus so to not charge egrigious
        /// amounts there is a cap. Note that there is also a cap enforced by
        /// the protocol so effectively the partner can only lower the
        /// limit here.
        max_volume_bps: u64,
    },
    /// Fees should be captured from an order's entire volume.
    /// In that case an order's execution must be so much better that
    /// taking a cut from the volume will not end up violating the
    /// order's limit price.
    Volume { bps: u64 },
}

impl Default for FeePolicy {
    fn default() -> Self {
        Self::Volume { bps: 0 }
    }
}

impl<'de> Deserialize<'de> for FeePolicy {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(untagged)]
        enum Helper {
            #[serde(rename_all = "camelCase")]
            Surplus {
                surplus_bps: u64,
                max_volume_bps: u64,
            },
            #[serde(rename_all = "camelCase")]
            PriceImprovement {
                price_improvement_bps: u64,
                max_volume_bps: u64,
            },
            #[serde(rename_all = "camelCase")]
            Volume { volume_bps: u64 },
            // Originally only volume fees were allowed and they used the field `bps`.
            // To stay backwards compatible with old appdata we still support this old
            // format.
            #[serde(rename_all = "camelCase")]
            VolumeOld { bps: u64 },
        }

        match Helper::deserialize(deserializer)? {
            Helper::Surplus {
                surplus_bps,
                max_volume_bps,
            } => Ok(FeePolicy::Surplus {
                bps: surplus_bps,
                max_volume_bps,
            }),
            Helper::PriceImprovement {
                price_improvement_bps,
                max_volume_bps,
            } => Ok(FeePolicy::PriceImprovement {
                bps: price_improvement_bps,
                max_volume_bps,
            }),
            Helper::Volume { volume_bps } => Ok(FeePolicy::Volume { bps: volume_bps }),
            Helper::VolumeOld { bps } => Ok(FeePolicy::Volume { bps }),
        }
    }
}

#[cfg(any(test, feature = "test_helpers"))]
impl serde::Serialize for FeePolicy {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        #[derive(Serialize)]
        #[serde(rename_all = "camelCase")]
        enum Helper {
            Surplus {
                surplus_bps: u64,
                max_volume_bps: u64,
            },
            PriceImprovement {
                price_improvement_bps: u64,
                max_volume_bps: u64,
            },
            Volume {
                volume_bps: u64,
            },
        }

        let helper = match self {
            Self::Volume { bps } => Helper::Volume { volume_bps: *bps },
            Self::Surplus {
                bps,
                max_volume_bps,
            } => Helper::Surplus {
                surplus_bps: *bps,
                max_volume_bps: *max_volume_bps,
            },
            Self::PriceImprovement {
                bps,
                max_volume_bps,
            } => Helper::PriceImprovement {
                price_improvement_bps: *bps,
                max_volume_bps: *max_volume_bps,
            },
        };

        helper.serialize(serializer)
    }
}

#[derive(Clone)]
pub struct Validator {
    /// App data size limit (in bytes).
    size_limit: usize,
}

#[cfg(any(test, feature = "test_helpers"))]
impl Default for Validator {
    fn default() -> Self {
        Self { size_limit: 8192 }
    }
}

impl Validator {
    /// Creates a new app data [`Validator`] with the provided app data
    /// `size_limit` (in bytes).
    pub fn new(size_limit: usize) -> Self {
        Self { size_limit }
    }

    /// Returns the app data size limit (in bytes).
    pub fn size_limit(&self) -> usize {
        self.size_limit
    }

    /// Parses and validates the provided app data bytes, returns the validated
    ///
    /// Valid app data is considered to be:
    /// 1. Below or equal to [`Validator::size_limit`] in size.
    /// 2. A valid JSON & app data object.
    pub fn validate(&self, full_app_data: &[u8]) -> Result<ValidatedAppData> {
        if full_app_data.len() > self.size_limit {
            return Err(anyhow!(
                "app data has byte size {} which is larger than limit {}",
                full_app_data.len(),
                self.size_limit
            ));
        }

        let document = String::from_utf8(full_app_data.to_vec())?;
        let protocol = parse(full_app_data)?;

        Ok(ValidatedAppData {
            hash: AppDataHash(hash_full_app_data(full_app_data)),
            document,
            protocol,
        })
    }
}

pub fn parse(full_app_data: &[u8]) -> Result<ProtocolAppData> {
    let root = serde_json::from_slice::<Root>(full_app_data).context("invalid app data json")?;
    let parsed = root
        .metadata
        .or_else(|| root.backend.map(ProtocolAppData::from))
        // If the key doesn't exist, default. Makes life easier for API
        // consumers, who don't care about protocol app data.
        .unwrap_or_default();
    Ok(parsed)
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
#[derive(Deserialize)]
#[cfg_attr(any(test, feature = "test_helpers"), derive(Clone, Serialize))]
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

/// Caches whether a given app data document contains wrappers, keyed by
/// hash. This avoids re-parsing the same JSON across orders and auction
/// cycles. We're using the default TinyLFU eviction policy, but the capacity is
/// large enough that we don't expect eviction to be a problem in practice, but
/// we limit the size to prevent potential memory exhaustion attacks.
pub struct WrapperCache(Cache<AppDataHash, bool>);

impl WrapperCache {
    pub fn new(capacity: u64) -> Self {
        Self(Cache::new(capacity))
    }

    /// Returns `true` if order appData contains non-empty wrappers
    pub fn has_wrappers(&self, hash: &AppDataHash, document: Option<&str>) -> bool {
        if let Some(cached) = self.0.get(hash) {
            return cached;
        }
        let result = document.is_some_and(|doc| {
            serde_json::from_str::<Root>(doc)
                .ok()
                .and_then(|root| root.metadata)
                .is_some_and(|m| !m.wrappers.is_empty())
        });
        self.0.insert(*hash, result);
        result
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
        const_hex::encode_to_slice(self.0.as_slice(), &mut bytes[2..]).unwrap();
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
                const_hex::decode_to_slice(s, value.as_mut()).map_err(|err| {
                    de::Error::custom(format!("failed to decode {s:?} as hex uid: {err}"))
                })?;
                Ok(OrderUid(value))
            }
        }

        deserializer.deserialize_str(Visitor {})
    }
}

/// A list containing all the partner fees
#[derive(Clone, Debug, Default, Eq, PartialEq)]
#[cfg_attr(
    any(test, feature = "test_helpers"),
    derive(Serialize),
    serde(transparent)
)]
pub struct PartnerFees(Vec<PartnerFee>);

impl PartnerFees {
    pub fn iter(&self) -> Iter<'_, PartnerFee> {
        self.0.iter()
    }
}

impl<'de> Deserialize<'de> for PartnerFees {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(untagged)]
        enum Helper {
            Single(PartnerFee),
            Multiple(Vec<PartnerFee>),
        }

        match Helper::deserialize(deserializer)? {
            Helper::Single(fee) => Ok(PartnerFees(vec![fee])),
            Helper::Multiple(fees) => Ok(PartnerFees(fees)),
        }
    }
}

/// The legacy `backend` app data object.
#[derive(Debug, Default, Deserialize)]
#[cfg_attr(any(test, feature = "test_helpers"), derive(Clone, Serialize))]
struct BackendAppData {
    #[serde(default)]
    pub hooks: Hooks,
}

impl From<BackendAppData> for ProtocolAppData {
    fn from(value: BackendAppData) -> Self {
        Self {
            hooks: value.hooks,
            wrappers: Vec::new(),
            signer: None,
            replaced_order: None,
            partner_fee: PartnerFees::default(),
            flashloan: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use {super::*, crate::Hook};

    macro_rules! assert_app_data {
        ($s:expr_2021, $e:expr_2021 $(,)?) => {{
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
                        target: Address::from_slice(&[0; 20]),
                        call_data: vec![],
                        gas_limit: 0,
                    }],
                    post: vec![
                        Hook {
                            target: Address::from_slice(&[1; 20]),
                            call_data: vec![1],
                            gas_limit: 1
                        },
                        Hook {
                            target: Address::from_slice(&[2; 20]),
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
                signer: Some(Address::from_slice(&[0x42; 20])),
                ..Default::default()
            },
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
                        "partnerFee": {
                            "bps": 100,
                            "recipient": "0x0202020202020202020202020202020202020202"
                        }
                    },
                    "version": "0.9.0"
                }
            "#,
            ProtocolAppData {
                partner_fee: PartnerFees(vec![PartnerFee {
                    policy: FeePolicy::Volume { bps: 100 },
                    recipient: Address::from_slice(&[2; 20]),
                }]),
                ..Default::default()
            },
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
                        "partnerFee": [
                            {
                                "bps": 100,
                                "recipient": "0x0202020202020202020202020202020202020202"
                            },
                            {
                                "volumeBps": 1000,
                                "recipient": "0x0101010101010101010101010101010101010101"
                            },
                            {
                                "surplusBps": 100,
                                "maxVolumeBps": 100,
                                "recipient": "0x0101010101010101010101010101010101010101"
                            },
                            {
                                "priceImprovementBps": 100,
                                "maxVolumeBps": 100,
                                "recipient": "0x0101010101010101010101010101010101010101"
                            }
                        ]
                    },
                    "version": "0.9.0"
                }
            "#,
            ProtocolAppData {
                partner_fee: PartnerFees(vec![
                    // this one was parsed from the old format for volume fees
                    PartnerFee {
                        policy: FeePolicy::Volume { bps: 100 },
                        recipient: Address::from_slice(&[2; 20]),
                    },
                    // this one is using the new format
                    PartnerFee {
                        policy: FeePolicy::Volume { bps: 1000 },
                        recipient: Address::from_slice(&[1; 20]),
                    },
                    PartnerFee {
                        policy: FeePolicy::Surplus {
                            bps: 100,
                            max_volume_bps: 100
                        },
                        recipient: Address::from_slice(&[1; 20]),
                    },
                    PartnerFee {
                        policy: FeePolicy::PriceImprovement {
                            bps: 100,
                            max_volume_bps: 100
                        },
                        recipient: Address::from_slice(&[1; 20]),
                    },
                ]),
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
                        target: Address::from_slice(&[0; 20]),
                        call_data: vec![],
                        gas_limit: 0,
                    }],
                    post: vec![
                        Hook {
                            target: Address::from_slice(&[1; 20]),
                            call_data: vec![1],
                            gas_limit: 1
                        },
                        Hook {
                            target: Address::from_slice(&[2; 20]),
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
    fn wrapper_cache_detects_wrappers() {
        let cache = WrapperCache::new(100);
        let h = |b: u8| AppDataHash([b; 32]);

        assert!(!cache.has_wrappers(&h(1), None));
        assert!(!cache.has_wrappers(&h(2), Some("{}")));
        assert!(!cache.has_wrappers(&h(3), Some(r#"{"metadata": {}}"#)));
        assert!(!cache.has_wrappers(&h(4), Some(r#"{"metadata": {"wrappers": []}}"#)));
        assert!(cache.has_wrappers(
            &h(5),
            Some(r#"{"metadata": {"wrappers": [{"address": "0x0000000000000000000000000000000000000001", "data": "0x"}]}}"#),
        ));

        // Second call hits the cache
        assert!(cache.has_wrappers(&h(5), None));
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
