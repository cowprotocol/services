use {
    model::{
        app_data::AppDataHash,
        bytes_hex,
        order::{BuyTokenDestination, OrderCreationAppData, SellTokenSource},
        quote::QuoteId,
        signature::{Signature, SigningScheme},
    },
    number::serialization::HexOrDecimalU256,
    primitive_types::{H160, U256},
    serde::{Deserialize, Serialize, Serializer},
    serde_with::serde_as,
    std::fmt::{self, Display},
    strum::EnumString,
    utoipa::{ToResponse, ToSchema},
};

#[derive(
    Eq, PartialEq, Clone, Copy, Debug, Default, Deserialize, Serialize, Hash, EnumString, ToSchema,
)]
#[strum(ascii_case_insensitive)]
#[serde(rename_all = "lowercase")]
pub enum OrderKind {
    #[default]
    Buy,
    Sell,
}

// uid as 56 bytes: 32 for orderDigest, 20 for ownerAddress and 4 for validTo
#[derive(Clone, Copy, Eq, Hash, PartialEq, PartialOrd, Ord, ToSchema, ToResponse)]
#[schema(example = json!("0xff2e2e54d178997f173266817c1e9ed6fee1a1aae4b43971c53b543cffcc2969845c6f5599fbb25dbdd1b9b013daf85c03f3c63763e4bc4a"))]
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

impl Serialize for OrderUid {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(self.to_string().as_str())
    }
}

#[serde_as]
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct OrderCreation {
    /// Address of token sold.
    #[schema(value_type = String, example = "0x6810e776880c02933d47db1b9fc05908e5386b96")]
    pub sell_token: H160,
    /// Address of token bought.
    #[schema(value_type = String, example = "0x6810e776880c02933d47db1b9fc05908e5386b96")]
    pub buy_token: H160,
    /// An optional address to receive the proceeds of the trade instead of the
    /// `owner` (i.e. the order signer).
    #[serde(default)]
    #[schema(value_type = Option<String>, example = "0x6810e776880c02933d47db1b9fc05908e5386b96")]
    pub receiver: Option<H160>,
    /// Amount of `sellToken` to be sold in atoms.
    #[serde_as(as = "HexOrDecimalU256")]
    #[schema(value_type = String, example = "1234567890")]
    pub sell_amount: U256,
    /// Amount of `buyToken` to be sold in atoms.
    #[serde_as(as = "HexOrDecimalU256")]
    #[schema(value_type = String, example = "1234567890")]
    pub buy_amount: U256,
    /// Unix timestamp (`uint32`) until which the order is valid.
    #[schema(example = 0)]
    pub valid_to: u32,
    /// feeRatio * sellAmount + minimal_fee in atoms.
    #[serde_as(as = "HexOrDecimalU256")]
    #[schema(value_type = String, example = "1234567890")]
    pub fee_amount: U256,
    /// Buy or sell?
    #[schema(example = OrderKind::Buy)]
    pub kind: OrderKind,
    /// Is the order fill-or-kill or partially fillable?
    #[schema(example = true)]
    pub partially_fillable: bool,
    #[serde(default)]
    #[schema(value_type = String, example = "erc20")]
    pub sell_token_balance: SellTokenSource,
    #[serde(default)]
    #[schema(value_type = String, example = "erc20")]
    pub buy_token_balance: BuyTokenDestination,
    /// If set, the backend enforces that this address matches what is decoded
    /// as the *signer* of the signature. This helps catch errors with
    /// invalid signature encodings as the backend might otherwise silently
    /// work with an unexpected address that for example does not have
    /// any balance.
    #[schema(value_type = Option<String>, example = "0x6810e776880c02933d47db1b9fc05908e5386b96")]
    pub from: Option<H160>,
    /// How was the order signed?
    #[schema(value_type = String, example = "eip712")]
    signing_scheme: SigningScheme,
    /// A signature.
    #[serde(with = "bytes_hex")]
    #[schema(value_type = String, example = "0x0000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000")]
    signature: Vec<u8>,
    /// Orders can optionally include a quote ID. This way the order can be
    /// linked to a quote and enable providing more metadata when analysing
    /// order slippage.
    #[schema(value_type = i64, example = 0)]
    pub quote_id: Option<QuoteId>,
    /// The string encoding of a JSON object representing some `appData`. The
    /// format of the JSON expected in the `appData` field is defined
    /// [here](https://github.com/cowprotocol/app-data).
    #[schema(value_type = String, example = "{\"version\":\"0.9.0\",\"metadata\":{}}")]
    pub app_data: String,
    #[schema(value_type = String, example = "0x0000000000000000000000000000000000000000000000000000000000000000")]
    pub app_data_hash: Option<String>,
}

impl TryFrom<OrderCreation> for model::order::OrderCreation {
    type Error = anyhow::Error;

    fn try_from(value: OrderCreation) -> Result<Self, Self::Error> {
        let signature = Signature::from_bytes(value.signing_scheme, &value.signature)?;

        let kind = match value.kind {
            OrderKind::Buy => model::order::OrderKind::Buy,
            OrderKind::Sell => model::order::OrderKind::Sell,
        };

        let app_data = match value.app_data_hash {
            Some(hash) => OrderCreationAppData::Both {
                full: value.app_data,
                expected: serde_json::from_str(&hash).unwrap(),
            },
            None => match serde_json::from_str::<AppDataHash>(&value.app_data) {
                Ok(deser) => OrderCreationAppData::Hash { hash: deser },
                Err(_) => OrderCreationAppData::Full {
                    full: value.app_data,
                },
            },
        };

        Ok(model::order::OrderCreation {
            sell_token: value.sell_token,
            buy_token: value.buy_token,
            receiver: value.receiver,
            sell_amount: value.sell_amount,
            buy_amount: value.buy_amount,
            valid_to: value.valid_to,
            fee_amount: value.fee_amount,
            kind,
            partially_fillable: value.partially_fillable,
            sell_token_balance: value.sell_token_balance,
            buy_token_balance: value.buy_token_balance,
            from: value.from,
            signature,
            quote_id: value.quote_id,
            app_data,
        })
    }
}
