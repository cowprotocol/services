use {
    super::serialize,
    number::serialization::HexOrDecimalU256,
    serde::Serialize,
    serde_json::Value,
    serde_with::serde_as,
    std::collections::HashMap,
    utoipa::{
        openapi::{
            AllOfBuilder,
            ArrayBuilder,
            ObjectBuilder,
            OneOfBuilder,
            Ref,
            RefOr,
            Schema,
            SchemaType,
        },
        ToSchema,
    },
    web3::types::{H160, U256},
};

/// Proposed solutions to settle some of the orders in the auction.
#[derive(Debug, Serialize, Default, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct Solutions {
    pub solutions: Vec<Solution>,
}

/// A computed solution for a given auction.
#[serde_as]
#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct Solution {
    /// An opaque identifier for the solution. This is a solver generated number
    /// that is unique across multiple solutions within the auction.
    #[schema(value_type = f64, format = Int64)]
    pub id: u64,
    /// A clearing price map of token address to price. The price can have
    /// arbitrary denomination.
    #[serde_as(as = "HashMap<_, HexOrDecimalU256>")]
    pub prices: HashMap<H160, U256>,
    /// CoW Protocol order trades included in the solution.
    pub trades: Vec<Trade>,
    /// Interactions to encode within a settlement.
    pub interactions: Vec<Interaction>,
    /// How many units of gas this solution is estimated to cost.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gas: Option<u64>,
}

#[derive(Debug, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum Trade {
    Fulfillment(Fulfillment),
    Jit(JitTrade),
}

impl ToSchema<'static> for Trade {
    fn schema() -> (&'static str, RefOr<Schema>) {
        (
            "Trade",
            Schema::OneOf(
                OneOfBuilder::new()
                    .description(Some(
                        "A trade for a CoW Protocol order included in a solution.",
                    ))
                    .item(Ref::from_schema_name("Fulfillment"))
                    .item(Ref::from_schema_name("JitTrade"))
                    .build(),
            )
            .into(),
        )
    }
}

#[serde_as]
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Fulfillment {
    #[serde_as(as = "serialize::Hex")]
    pub order: [u8; 56],
    #[serde_as(as = "HexOrDecimalU256")]
    pub executed_amount: U256,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde_as(as = "Option<HexOrDecimalU256>")]
    pub fee: Option<U256>,
}

impl ToSchema<'static> for Fulfillment {
    fn schema() -> (&'static str, RefOr<Schema>) {
        (
            "Fulfillment",
            Schema::Object(
                ObjectBuilder::new()
                    .description(Some("A trade which fulfills an order from the auction."))
                    .required("kind")
                    .required("order")
                    .property(
                        "kind",
                        ObjectBuilder::new()
                            .schema_type(SchemaType::String)
                            .enum_values(Some(["fulfillment"])),
                    )
                    .property(
                        "order",
                        AllOfBuilder::new()
                            .item(Ref::from_schema_name("OrderUid"))
                            .description(Some(
                                "A reference by UID of the order to execute in a solution. The \
                                 order must be included in the auction input.",
                            )),
                    )
                    .property(
                        "executedAmount",
                        AllOfBuilder::new()
                            .description(Some(
                                "The amount of the order that was executed. This is denoted in \
                                 'sellToken' for sell orders, and 'buyToken' for buy orders.",
                            ))
                            .item(Ref::from_schema_name("TokenAmount")),
                    )
                    .property(
                        "fee",
                        ObjectBuilder::new().description(Some(
                            "The sell token amount that should be taken as a fee for this trade. \
                             This only gets returned for limit orders and only refers to the \
                             actual amount filled by the trade.",
                        )),
                    )
                    .build(),
            )
            .into(),
        )
    }
}

#[serde_as]
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct JitTrade {
    pub order: JitOrder,
    #[serde_as(as = "HexOrDecimalU256")]
    pub executed_amount: U256,
}

impl ToSchema<'static> for JitTrade {
    fn schema() -> (&'static str, RefOr<Schema>) {
        (
            "JitTrade",
            Schema::Object(
                ObjectBuilder::new()
                    .description(Some("A trade with a JIT order."))
                    .required("kind")
                    .required("order")
                    .required("executedAmount")
                    .property(
                        "kind",
                        ObjectBuilder::new()
                            .schema_type(SchemaType::String)
                            .enum_values(Some(["jit"])),
                    )
                    .property(
                        "order",
                        AllOfBuilder::new()
                            .description(Some(
                                "The just-in-time liquidity order to execute in a solution.",
                            ))
                            .item(Ref::from_schema_name("JitOrder")),
                    )
                    .property(
                        "executedAmount",
                        AllOfBuilder::new()
                            .description(Some(
                                "The amount of the order that was executed. This is denoted in \
                                 'sellToken' for sell orders, and 'buyToken' for buy orders.",
                            ))
                            .item(Ref::from_schema_name("TokenAmount")),
                    )
                    .build(),
            )
            .into(),
        )
    }
}

/// A just-in-time liquidity order included in a settlement.
#[serde_as]
#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct JitOrder {
    #[schema(value_type = Token)]
    pub sell_token: H160,
    #[schema(value_type = Token)]
    pub buy_token: H160,
    #[schema(value_type = Address)]
    pub receiver: H160,
    #[serde_as(as = "HexOrDecimalU256")]
    #[schema(value_type = TokenAmount)]
    pub sell_amount: U256,
    #[serde_as(as = "HexOrDecimalU256")]
    #[schema(value_type = TokenAmount)]
    pub buy_amount: U256,
    pub valid_to: u32,
    #[serde_as(as = "serialize::Hex")]
    #[schema(value_type = AppData)]
    pub app_data: [u8; 32],
    #[serde_as(as = "HexOrDecimalU256")]
    #[schema(value_type = TokenAmount)]
    pub fee_amount: U256,
    pub kind: OrderKind,
    pub partially_fillable: bool,
    pub sell_token_balance: SellTokenBalance,
    pub buy_token_balance: BuyTokenBalance,
    pub signing_scheme: SigningScheme,
    #[serde_as(as = "serialize::Hex")]
    #[schema(value_type = Signature)]
    pub signature: Vec<u8>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum OrderKind {
    Sell,
    Buy,
}

#[derive(Debug, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum Interaction {
    Liquidity(LiquidityInteraction),
    Custom(CustomInteraction),
}

// todo: Currently, it strictly follows the manual api schema. This has to be
// automated and deleted.
impl ToSchema<'static> for Interaction {
    fn schema() -> (&'static str, RefOr<Schema>) {
        (
            "Interaction",
            Schema::AllOf(
                AllOfBuilder::new()
                    .description(Some("An interaction to execute as part of a settlement."))
                    .item(
                        ObjectBuilder::new().property(
                            "internalize",
                            ObjectBuilder::new()
                                .schema_type(SchemaType::Boolean)
                                .description(Some(
                                    "A flag indicating that the interaction should be \
                                     'internalized', as specified by CIP-2.",
                                ))
                                .build(),
                        ),
                    )
                    .item(
                        OneOfBuilder::new()
                            .item(Ref::from_schema_name("LiquidityInteraction"))
                            .item(Ref::from_schema_name("CustomInteraction")),
                    )
                    .build(),
            )
            .into(),
        )
    }
}

#[derive(Debug, Serialize)]
pub enum InteractionType {
    Liquidity(LiquidityInteraction),
    Custom(CustomInteraction),
}

#[serde_as]
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LiquidityInteraction {
    pub internalize: bool,
    /// The ID of executed liquidity provided in the auction input.
    pub id: String,
    pub input_token: H160,
    pub output_token: H160,
    #[serde_as(as = "HexOrDecimalU256")]
    pub input_amount: U256,
    #[serde_as(as = "HexOrDecimalU256")]
    pub output_amount: U256,
}

impl ToSchema<'static> for LiquidityInteraction {
    fn schema() -> (&'static str, RefOr<Schema>) {
        (
            "LiquidityInteraction",
            Schema::Object(
                ObjectBuilder::new()
                    .description(Some(
                        "Interaction representing the execution of liquidity that was passed in \
                         with the auction.",
                    ))
                    .required("kind")
                    .required("id")
                    .required("inputToken")
                    .required("outputToken")
                    .required("inputAmount")
                    .required("outputAmount")
                    .property(
                        "kind",
                        ObjectBuilder::new()
                            .schema_type(SchemaType::String)
                            .enum_values(Some(["liquidity"])),
                    )
                    .property(
                        "id",
                        ObjectBuilder::new()
                            .schema_type(SchemaType::String)
                            .description(Some(
                                "The ID of executed liquidity provided in the auction input.",
                            ))
                            .build(),
                    )
                    .property("inputToken", Ref::from_schema_name("Token"))
                    .property("outputToken", Ref::from_schema_name("Token"))
                    .property("inputAmount", Ref::from_schema_name("TokenAmount"))
                    .property("outputAmount", Ref::from_schema_name("TokenAmount"))
                    .build(),
            )
            .into(),
        )
    }
}

#[serde_as]
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CustomInteraction {
    pub internalize: bool,
    pub target: H160,
    #[serde_as(as = "HexOrDecimalU256")]
    pub value: U256,
    /// The EVM calldata bytes.
    #[serde(rename = "callData")]
    #[serde_as(as = "serialize::Hex")]
    pub calldata: Vec<u8>,
    /// ERC20 allowances that are required for this custom interaction.
    pub allowances: Vec<Allowance>,
    pub inputs: Vec<Asset>,
    pub outputs: Vec<Asset>,
}

impl ToSchema<'static> for CustomInteraction {
    fn schema() -> (&'static str, RefOr<Schema>) {
        (
            "CustomInteraction",
            Schema::Object(
                ObjectBuilder::new()
                    .description(Some(
                        "A searcher-specified custom interaction to be included in the final \
                         settlement.",
                    ))
                    .required("kind")
                    .required("target")
                    .required("value")
                    .required("callData")
                    .required("inputs")
                    .required("outputs")
                    .property(
                        "kind",
                        ObjectBuilder::new()
                            .schema_type(SchemaType::String)
                            .enum_values(Some(["custom"])),
                    )
                    .property("target", Ref::from_schema_name("Address"))
                    .property("value", Ref::from_schema_name("TokenAmount"))
                    .property(
                        "callData",
                        ObjectBuilder::new()
                            .schema_type(SchemaType::String)
                            .description(Some("The EVM calldata bytes."))
                            .example(Some(Value::String("0x01020304".to_string())))
                            .build(),
                    )
                    .property(
                        "allowances",
                        ArrayBuilder::new()
                            .items(Ref::from_schema_name("Allowance"))
                            .description(Some(
                                "ERC20 allowances that are required for this custom interaction.",
                            ))
                            .build(),
                    )
                    .property(
                        "inputs",
                        ArrayBuilder::new()
                            .items(Ref::from_schema_name("Asset"))
                            .build(),
                    )
                    .property(
                        "outputs",
                        ArrayBuilder::new()
                            .items(Ref::from_schema_name("Asset"))
                            .build(),
                    )
                    .build(),
            )
            .into(),
        )
    }
}

/// A token address with an amount.
#[serde_as]
#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct Asset {
    #[schema(value_type = Token)]
    pub token: H160,
    #[schema(value_type = TokenAmount)]
    #[serde_as(as = "HexOrDecimalU256")]
    pub amount: U256,
}

/// An ERC20 allowance from the settlement contract to some spender that is
/// required for a custom interaction.
#[serde_as]
#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct Allowance {
    #[schema(value_type = Token)]
    pub token: H160,
    #[schema(value_type = Address)]
    pub spender: H160,
    #[schema(value_type = TokenAmount)]
    #[serde_as(as = "HexOrDecimalU256")]
    pub amount: U256,
}

#[derive(Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum SellTokenBalance {
    #[default]
    Erc20,
    Internal,
    External,
}

impl ToSchema<'static> for SellTokenBalance {
    fn schema() -> (&'static str, RefOr<Schema>) {
        (
            "SellTokenBalance",
            Schema::Object(
                ObjectBuilder::new()
                    .description(Some("Where should the sell token be drawn from?"))
                    .schema_type(SchemaType::String)
                    .enum_values(Some(["erc20", "internal", "external"]))
                    .build(),
            )
            .into(),
        )
    }
}

#[derive(Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum BuyTokenBalance {
    #[default]
    Erc20,
    Internal,
}

impl ToSchema<'static> for BuyTokenBalance {
    fn schema() -> (&'static str, RefOr<Schema>) {
        (
            "BuyTokenBalance",
            Schema::Object(
                ObjectBuilder::new()
                    .description(Some("Where should the buy token be transferred to?"))
                    .schema_type(SchemaType::String)
                    .enum_values(Some(["erc20", "internal"]))
                    .build(),
            )
            .into(),
        )
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum SigningScheme {
    Eip712,
    EthSign,
    PreSign,
    Eip1271,
}

impl ToSchema<'static> for SigningScheme {
    fn schema() -> (&'static str, RefOr<Schema>) {
        (
            "SigningScheme",
            Schema::Object(
                ObjectBuilder::new()
                    .description(Some("How was the order signed?"))
                    .schema_type(SchemaType::String)
                    .enum_values(Some(["eip712", "ethSign", "preSign", "eip1271"]))
                    .build(),
            )
            .into(),
        )
    }
}
