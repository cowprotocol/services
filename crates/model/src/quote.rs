use {
    crate::{
        app_data::AppDataHash,
        order::{BuyTokenDestination, OrderCreationAppData, OrderKind, SellTokenSource},
        signature::SigningScheme,
        time,
    },
    anyhow::bail,
    chrono::{DateTime, Utc},
    number::nonzero::U256 as NonZeroU256,
    primitive_types::H160,
    serde::{de, ser::SerializeStruct as _, Deserialize, Deserializer, Serialize, Serializer},
    serde_with::serde_as,
};

#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum PriceQuality {
    /// We pick the best quote of the fastest `n` price estimators.
    Fast,
    #[default]
    /// We pick the best quote of all price estimators.
    Optimal,
    /// Quotes may by discarde when they failed to be verified by simulation.
    Verified,
}

#[derive(Eq, PartialEq, Clone, Copy, Debug, Default, Deserialize, Serialize, Hash)]
#[serde(
    rename_all = "lowercase",
    tag = "signingScheme",
    try_from = "QuoteSigningDeserializationData"
)]
pub enum QuoteSigningScheme {
    #[default]
    Eip712,
    EthSign,
    Eip1271 {
        #[serde(rename = "onchainOrder")]
        onchain_order: bool,
        #[serde(
            rename = "verificationGasLimit",
            default = "default_verification_gas_limit"
        )]
        verification_gas_limit: u64,
    },
    PreSign {
        #[serde(rename = "onchainOrder")]
        onchain_order: bool,
    },
}

impl QuoteSigningScheme {
    /// Returns the additional gas amount associated with a signing scheme.
    pub fn additional_gas_amount(&self) -> u64 {
        match self {
            QuoteSigningScheme::Eip1271 {
                verification_gas_limit,
                ..
            } => *verification_gas_limit,
            _ => 0u64,
        }
    }

    pub fn new_eip1271_with_default_gas(onchain_order: bool) -> Self {
        QuoteSigningScheme::Eip1271 {
            onchain_order,
            verification_gas_limit: default_verification_gas_limit(),
        }
    }
}

#[serde_as]
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct QuoteSigningDeserializationData {
    #[serde(default)]
    signing_scheme: SigningScheme,
    #[serde(default)]
    verification_gas_limit: Option<u64>,
    #[serde(default)]
    onchain_order: bool,
}

pub fn default_verification_gas_limit() -> u64 {
    // default gas limit is based Ambire usecase. See here:
    // https://github.com/cowprotocol/services/pull/480#issuecomment-1273190380
    27_000_u64
}

impl TryFrom<QuoteSigningDeserializationData> for QuoteSigningScheme {
    type Error = anyhow::Error;

    fn try_from(data: QuoteSigningDeserializationData) -> Result<Self, Self::Error> {
        match (
            data.signing_scheme,
            data.onchain_order,
            data.verification_gas_limit,
        ) {
            (scheme, true, None) if scheme.is_ecdsa_scheme() => {
                bail!("ECDSA-signed orders cannot be on-chain")
            }
            (SigningScheme::Eip712, _, None) => Ok(Self::Eip712),
            (SigningScheme::EthSign, _, None) => Ok(Self::EthSign),
            (SigningScheme::Eip1271, onchain_order, verification_gas_limit) => Ok(Self::Eip1271 {
                onchain_order,
                verification_gas_limit: verification_gas_limit
                    .unwrap_or_else(default_verification_gas_limit),
            }),
            (SigningScheme::PreSign, onchain_order, None) => Ok(Self::PreSign { onchain_order }),
            (_, _, Some(_)) => {
                bail!("Only EIP-1271 signatures should have a verification_gas_limit!")
            }
        }
    }
}

/// The order parameters to quote a price and fee for.
#[derive(Clone, Debug, Default, Deserialize, Serialize, Eq, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct OrderQuoteRequest {
    pub from: H160,
    pub sell_token: H160,
    pub buy_token: H160,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub receiver: Option<H160>,
    #[serde(flatten)]
    pub side: OrderQuoteSide,
    #[serde(flatten)]
    pub validity: Validity,
    #[serde(flatten, deserialize_with = "deserialize_optional_app_data")]
    pub app_data: OrderCreationAppData,
    #[serde(default)]
    pub sell_token_balance: SellTokenSource,
    #[serde(default)]
    pub buy_token_balance: BuyTokenDestination,
    #[serde(flatten)]
    pub signing_scheme: QuoteSigningScheme,
    #[serde(default)]
    pub price_quality: PriceQuality,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, Eq, PartialEq)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum OrderQuoteSide {
    #[serde(rename_all = "camelCase")]
    Sell {
        #[serde(flatten)]
        sell_amount: SellAmount,
    },
    #[serde(rename_all = "camelCase")]
    Buy { buy_amount_after_fee: NonZeroU256 },
}

impl Default for OrderQuoteSide {
    fn default() -> Self {
        Self::Buy {
            buy_amount_after_fee: NonZeroU256::one(),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Validity {
    To(u32),
    For(u32),
}

impl Validity {
    /// Returns a materialized valid-to value for the specified validity.
    pub fn actual_valid_to(self) -> u32 {
        match self {
            Validity::To(valid_to) => valid_to,
            Validity::For(valid_for) => time::now_in_epoch_seconds().saturating_add(valid_for),
        }
    }
}

impl Default for Validity {
    fn default() -> Self {
        // use the default CowSwap validity of 30 minutes.
        Self::For(30 * 60)
    }
}

/// Helper struct for `Validity` serialization.

impl<'de> Deserialize<'de> for Validity {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(rename = "validity", rename_all = "camelCase")]
        struct Helper {
            valid_to: Option<u32>,
            valid_for: Option<u32>,
        }

        let data = Helper::deserialize(deserializer)?;
        match (data.valid_to, data.valid_for) {
            (Some(valid_to), None) => Ok(Self::To(valid_to)),
            (None, Some(valid_for)) => Ok(Self::For(valid_for)),
            (None, None) => Ok(Self::default()),
            _ => Err(de::Error::custom(
                "must specify at most one of `validTo` or `validFor`",
            )),
        }
    }
}

impl Serialize for Validity {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let (field, value) = match self {
            Self::To(valid_to) => ("validTo", valid_to),
            Self::For(valid_for) => ("validFor", valid_for),
        };

        let mut ser = serializer.serialize_struct("Validity", 1)?;
        ser.serialize_field(field, value)?;
        ser.end()
    }
}

/// Manual `Deserialize` implementation for OrderCreationAppData that allows for
/// `appData` to be omitted. This is needed because `#[serde(default, flatten)]`
/// does not work as expected and will generate errors for quotes without
/// `appData` specified.
fn deserialize_optional_app_data<'de, D>(deserializer: D) -> Result<OrderCreationAppData, D::Error>
where
    D: Deserializer<'de>,
{
    #[derive(Deserialize)]
    #[serde(rename = "appData", rename_all = "camelCase")]
    struct Helper {
        app_data: Option<String>,
        app_data_hash: Option<AppDataHash>,
    }

    let data = Helper::deserialize(deserializer)?;
    let result = match (data.app_data, data.app_data_hash) {
        (Some(app_data), None) => match app_data.parse() {
            Ok(hash) => OrderCreationAppData::Hash { hash },
            Err(_) => OrderCreationAppData::Full { full: app_data },
        },
        (Some(full), Some(expected)) => OrderCreationAppData::Both { full, expected },
        (None, None) => OrderCreationAppData::default(),
        _ => return Err(de::Error::custom("invalid app data")),
    };

    Ok(result)
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, Eq, PartialEq)]
#[serde(untagged)]
pub enum SellAmount {
    BeforeFee {
        #[serde(rename = "sellAmountBeforeFee")]
        value: NonZeroU256,
    },
    AfterFee {
        #[serde(rename = "sellAmountAfterFee")]
        value: NonZeroU256,
    },
}

/// The quoted order by the service.
#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OrderQuote {
    pub sell_token: H160,
    pub buy_token: H160,
    pub receiver: Option<H160>,
    pub sell_amount: number::U256,
    pub buy_amount: number::U256,
    pub valid_to: u32,
    #[serde(flatten)]
    pub app_data: OrderCreationAppData,
    pub fee_amount: number::U256,
    pub kind: OrderKind,
    pub partially_fillable: bool,
    pub sell_token_balance: SellTokenSource,
    pub buy_token_balance: BuyTokenDestination,
    pub signing_scheme: SigningScheme,
}

pub type QuoteId = i64;

#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OrderQuoteResponse {
    pub quote: OrderQuote,
    pub from: H160,
    pub expiration: DateTime<Utc>,
    pub id: Option<QuoteId>,
    pub verified: bool,
}

impl OrderQuoteRequest {
    /// This method is used by the old, deprecated, fee endpoint to convert
    /// {Buy, Sell}Requests
    pub fn new(sell_token: H160, buy_token: H160, side: OrderQuoteSide) -> Self {
        Self {
            sell_token,
            buy_token,
            side,
            ..Default::default()
        }
    }
}

#[cfg(test)]
mod tests {
    use {super::*, serde_json::json};

    #[test]
    fn serialize_defaults() {
        assert_eq!(
            json!(OrderQuoteRequest::default()),
            json!({
                "from": "0x0000000000000000000000000000000000000000",
                "sellToken": "0x0000000000000000000000000000000000000000",
                "buyToken": "0x0000000000000000000000000000000000000000",
                "kind": "buy",
                "buyAmountAfterFee": "1",
                "validFor": 1800,
                "appData": "0x0000000000000000000000000000000000000000000000000000000000000000",
                "sellTokenBalance": "erc20",
                "buyTokenBalance": "erc20",
                "signingScheme": "eip712",
                "priceQuality": "optimal",
            })
        );
    }

    #[test]
    fn deserialize_quote_requests() {
        let valid_json = [
            json!({
                "from": "0x0000000000000000000000000000000000000000",
                "sellToken": "0x0000000000000000000000000000000000000001",
                "buyToken": "0x0000000000000000000000000000000000000002",
                "kind": "buy",
                "buyAmountAfterFee": "1",
            }),
            json!({
                "from": "0x0000000000000000000000000000000000000000",
                "sellToken": "0x0000000000000000000000000000000000000001",
                "buyToken": "0x0000000000000000000000000000000000000002",
                "kind": "buy",
                "buyAmountAfterFee": "1",
                "signingScheme": "eip712",
            }),
            json!({
                "from": "0x0000000000000000000000000000000000000000",
                "sellToken": "0x0000000000000000000000000000000000000001",
                "buyToken": "0x0000000000000000000000000000000000000002",
                "kind": "buy",
                "buyAmountAfterFee": "1",
                "signingScheme": "ethsign",
                "onchainOrder": false,
            }),
            json!({
                "from": "0x0000000000000000000000000000000000000000",
                "sellToken": "0x0000000000000000000000000000000000000001",
                "buyToken": "0x0000000000000000000000000000000000000002",
                "kind": "buy",
                "buyAmountAfterFee": "1",
                "signingScheme": "eip1271",
                "onchainOrder": true,
            }),
            json!({
                "from": "0x0000000000000000000000000000000000000000",
                "sellToken": "0x0000000000000000000000000000000000000001",
                "buyToken": "0x0000000000000000000000000000000000000002",
                "kind": "buy",
                "buyAmountAfterFee": "1",
                "signingScheme": "eip1271",
                "onchainOrder": true,
                "verificationGasLimit": 10000000
            }),
            json!({
                "from": "0x0000000000000000000000000000000000000000",
                "sellToken": "0x0000000000000000000000000000000000000001",
                "buyToken": "0x0000000000000000000000000000000000000002",
                "kind": "buy",
                "buyAmountAfterFee": "1",
                "signingScheme": "eip1271",
            }),
            json!({
                "from": "0x0000000000000000000000000000000000000000",
                "sellToken": "0x0000000000000000000000000000000000000001",
                "buyToken": "0x0000000000000000000000000000000000000002",
                "kind": "buy",
                "buyAmountAfterFee": "1",
                "signingScheme": "presign",
                "onchainOrder": true,
            }),
            json!({
                "from": "0x0000000000000000000000000000000000000000",
                "sellToken": "0x0000000000000000000000000000000000000001",
                "buyToken": "0x0000000000000000000000000000000000000002",
                "kind": "buy",
                "buyAmountAfterFee": "1",
                "signingScheme":  "presign",
            }),
            json!({
                "from": "0x0000000000000000000000000000000000000000",
                "sellToken": "0x0000000000000000000000000000000000000001",
                "buyToken": "0x0000000000000000000000000000000000000002",
                "kind": "buy",
                "buyAmountAfterFee": "1",
                "appData": "0x1111111111111111111111111111111111111111111111111111111111111111",
            }),
            json!({
                "from": "0x0000000000000000000000000000000000000000",
                "sellToken": "0x0000000000000000000000000000000000000001",
                "buyToken": "0x0000000000000000000000000000000000000002",
                "kind": "buy",
                "buyAmountAfterFee": "1",
                "appData": "1",
            }),
            json!({
                "from": "0x0000000000000000000000000000000000000000",
                "sellToken": "0x0000000000000000000000000000000000000001",
                "buyToken": "0x0000000000000000000000000000000000000002",
                "kind": "buy",
                "buyAmountAfterFee": "1",
                "appData": "2",
                "appDataHash": "0x2222222222222222222222222222222222222222222222222222222222222222",
            }),
        ];
        let expected_standard_response = OrderQuoteRequest {
            sell_token: H160::from_low_u64_be(1),
            buy_token: H160::from_low_u64_be(2),
            ..Default::default()
        };
        let modify_signing_scheme = |signing_scheme: QuoteSigningScheme| {
            let mut response = expected_standard_response.clone();
            response.signing_scheme = signing_scheme;
            response
        };
        let expected_quote_responses = vec![
            expected_standard_response.clone(),
            expected_standard_response.clone(),
            modify_signing_scheme(QuoteSigningScheme::EthSign),
            modify_signing_scheme(QuoteSigningScheme::Eip1271 {
                onchain_order: true,
                verification_gas_limit: default_verification_gas_limit(),
            }),
            modify_signing_scheme(QuoteSigningScheme::Eip1271 {
                onchain_order: true,
                verification_gas_limit: 10000000u64,
            }),
            modify_signing_scheme(QuoteSigningScheme::Eip1271 {
                onchain_order: false,
                verification_gas_limit: default_verification_gas_limit(),
            }),
            modify_signing_scheme(QuoteSigningScheme::PreSign {
                onchain_order: true,
            }),
            modify_signing_scheme(QuoteSigningScheme::PreSign {
                onchain_order: false,
            }),
            OrderQuoteRequest {
                app_data: OrderCreationAppData::Hash {
                    hash: AppDataHash([0x11; 32]),
                },
                ..expected_standard_response.clone()
            },
            OrderQuoteRequest {
                app_data: OrderCreationAppData::Full {
                    full: "1".to_string(),
                },
                ..expected_standard_response.clone()
            },
            OrderQuoteRequest {
                app_data: OrderCreationAppData::Both {
                    full: "2".to_string(),
                    expected: AppDataHash([0x22; 32]),
                },
                ..expected_standard_response.clone()
            },
        ];
        for (i, json) in valid_json.iter().enumerate() {
            assert_eq!(
                serde_json::from_value::<OrderQuoteRequest>(json.clone()).unwrap(),
                *expected_quote_responses.get(i).unwrap()
            );
        }
        let invalid_json = vec![
            json!({
                "from": "0x0000000000000000000000000000000000000000",
                "sellToken": "0x0000000000000000000000000000000000000001",
                "buyToken": "0x0000000000000000000000000000000000000002",
                "kind": "buy",
                "buyAmountAfterFee": "1",
                "onchainOrder": true,
            }),
            json!({
                "from": "0x0000000000000000000000000000000000000000",
                "sellToken": "0x0000000000000000000000000000000000000001",
                "buyToken": "0x0000000000000000000000000000000000000002",
                "kind": "buy",
                "buyAmountAfterFee": "1",
                "signingScheme": "eip712",
                "onchainOrder": true,
            }),
            json!({
                "from": "0x0000000000000000000000000000000000000000",
                "sellToken": "0x0000000000000000000000000000000000000001",
                "buyToken": "0x0000000000000000000000000000000000000002",
                "kind": "buy",
                "buyAmountAfterFee": "1",
                "signingScheme": "ethsign",
                "onchainOrder": true,
            }),
            // `appDataHash` cannot be specified without a `appData` pre-image.
            json!({
                "from": "0x0000000000000000000000000000000000000000",
                "sellToken": "0x0000000000000000000000000000000000000001",
                "buyToken": "0x0000000000000000000000000000000000000002",
                "kind": "buy",
                "buyAmountAfterFee": "1",
                "appDataHash": "0x0000000000000000000000000000000000000000000000000000000000000000",
            }),
        ];
        let expected_errors = [
            "ECDSA-signed orders cannot be on-chain",
            "ECDSA-signed orders cannot be on-chain",
            "ECDSA-signed orders cannot be on-chain",
            "invalid app data",
        ];
        for (i, json) in invalid_json.iter().enumerate() {
            assert_eq!(
                serde_json::from_value::<OrderQuoteRequest>(json.clone())
                    .unwrap_err()
                    .to_string(),
                expected_errors[i],
            );
        }
    }
}
