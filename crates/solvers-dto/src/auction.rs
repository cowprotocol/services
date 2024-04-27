use {
    super::serialize,
    app_data::AppDataHash,
    bigdecimal::BigDecimal,
    number::serialization::HexOrDecimalU256,
    serde::Deserialize,
    serde_json::{Number, Value},
    serde_with::{serde_as, DisplayFromStr},
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
            SchemaType::{self},
        },
        ToSchema,
    },
    web3::types::{H160, H256, U256},
};

/// The abstract auction to be solved by the searcher.
#[serde_as]
#[derive(Debug, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct Auction {
    /// An opaque identifier for the auction. Will be set to `null` for requests
    /// that are not part of an auction (when quoting token prices for example).
    #[serde_as(as = "Option<DisplayFromStr>")]
    #[schema(value_type = String)]
    pub id: Option<i64>,
    /// A map of token addresses to token information.
    pub tokens: HashMap<H160, TokenInfo>,
    /// The solvable orders included in the auction.
    pub orders: Vec<Order>,
    /// On-chain liquidity that can be used by the solution.
    pub liquidity: Vec<Liquidity>,
    /// The current estimated gas price that will be paid when executing a
    /// settlement. Additionally, this is the gas price that is multiplied with
    /// a settlement's gas estimate for solution scoring.
    #[serde_as(as = "HexOrDecimalU256")]
    #[schema(value_type = TokenAmount)]
    pub effective_gas_price: U256,
    /// The deadline by which a solution to the auction is required. Requests
    /// that go beyond this deadline are expected to be cancelled by the caller.
    #[schema(value_type = DateTime)]
    pub deadline: chrono::DateTime<chrono::Utc>,
}

/// CoW Protocol order information relevant to execution.
#[serde_as]
#[derive(Debug, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct Order {
    #[schema(value_type = OrderUid)]
    #[serde_as(as = "serialize::Hex")]
    pub uid: [u8; 56],
    #[schema(value_type = Token)]
    pub sell_token: H160,
    #[schema(value_type = Token)]
    pub buy_token: H160,
    #[schema(value_type = TokenAmount)]
    #[serde_as(as = "HexOrDecimalU256")]
    pub sell_amount: U256,
    #[serde_as(as = "HexOrDecimalU256")]
    pub full_sell_amount: U256,
    #[schema(value_type = TokenAmount)]
    #[serde_as(as = "HexOrDecimalU256")]
    pub buy_amount: U256,
    #[serde_as(as = "HexOrDecimalU256")]
    pub full_buy_amount: U256,
    pub fee_policies: Option<Vec<FeePolicy>>,
    pub valid_to: u32,
    pub kind: OrderKind,
    pub receiver: Option<H160>,
    pub owner: H160,
    /// Whether or not this order can be partially filled. If this is false,
    /// then the order is a "fill-or-kill" order, meaning it needs to be
    /// completely filled or not at all.
    pub partially_fillable: bool,
     pub pre_interactions: Vec<InteractionData>,
    pub post_interactions: Vec<InteractionData>,
    pub sell_token_source: SellTokenSource,
    pub buy_token_destination: BuyTokenDestination,
    pub class: OrderClass,
    pub app_data: AppDataHash,
    pub signing_scheme: SigningScheme,
    #[serde(with = "bytes_hex")]
    pub signature: Vec<u8>,
}

/// Destination for which the buyAmount should be transferred to order's
/// receiver to upon fulfillment
#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BuyTokenDestination {
    /// Pay trade proceeds as an ERC20 token transfer
    Erc20,
    /// Pay trade proceeds as a Vault internal balance transfer
    Internal,
}

/// Source from which the sellAmount should be drawn upon order fulfillment
#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SellTokenSource {
    /// Direct ERC20 allowances to the Vault relayer contract
    Erc20,
    /// Internal balances to the Vault with GPv2 relayer approval
    External,
    /// ERC20 allowances to the Vault with GPv2 relayer approval
    Internal,
}

#[serde_as]
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InteractionData {
    pub target: H160,
    #[serde_as(as = "HexOrDecimalU256")]
    pub value: U256,
    #[serde(with = "bytes_hex")]
    pub call_data: Vec<u8>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SigningScheme {
    Eip712,
    EthSign,
    Eip1271,
    PreSign,
}

/// How the CoW Protocol order was classified.
#[derive(Debug, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub enum OrderKind {
    Sell,
    Buy,
}

/// How the CoW Protocol order was classified.
#[derive(Debug, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub enum OrderClass {
    Market,
    Limit,
    Liquidity,
}

/// A fee policy that applies to an order.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum FeePolicy {
    /// If the order receives more than limit price, pay the protocol a factor
    /// of the difference.
    #[serde(rename_all = "camelCase")]
    Surplus {
        /// The factor of the user surplus that the protocol will request from
        /// the solver after settling the order
        factor: f64,
        /// Never charge more than that percentage of the order volume.
        max_volume_factor: f64,
    },
    /// A cut from the price improvement over the best quote is taken as a
    /// protocol fee.
    #[serde(rename_all = "camelCase")]
    PriceImprovement {
        /// The factor of the user surplus that the protocol will request from
        /// the solver after settling the order.
        factor: f64,
        /// Never charge more than that percentage of the order volume.
        max_volume_factor: f64,
        quote: Quote,
    },
    #[serde(rename_all = "camelCase")]
    Volume { factor: f64 },
}

impl ToSchema<'static> for FeePolicy {
    fn schema() -> (&'static str, RefOr<Schema>) {
        (
            "FeePolicy",
            Schema::OneOf(
                OneOfBuilder::new()
                    .description(Some("A fee policy that applies to an order"))
                    .item(Ref::from_schema_name("SurplusFee"))
                    .item(Ref::from_schema_name("PriceImprovement"))
                    .item(Ref::from_schema_name("VolumeFee"))
                    .build(),
            )
            .into(),
        )
    }
}

#[serde_as]
#[derive(Debug, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct Quote {
    #[serde_as(as = "HexOrDecimalU256")]
    #[schema(value_type = TokenAmount)]
    pub sell_amount: U256,
    #[serde_as(as = "HexOrDecimalU256")]
    #[schema(value_type = TokenAmount)]
    pub buy_amount: U256,
    #[serde_as(as = "HexOrDecimalU256")]
    #[schema(value_type = TokenAmount)]
    pub fee: U256,
}

/// Information about a token relevant to the auction.
#[serde_as]
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TokenInfo {
    /// The ERC20.decimals value for this token. This may be missing for ERC20
    /// tokens that don't implement the optional metadata extension.
    pub decimals: Option<u8>,
    /// The ERC20.symbol value for this token. This may be missing for ERC20
    /// tokens that don't implement the optional metadata extension.
    pub symbol: Option<String>,
    /// The reference price of this token for the auction used for scoring. This
    /// price is only included for tokens for which there are CoW Protocol
    /// orders.
    #[serde_as(as = "Option<HexOrDecimalU256>")]
    pub reference_price: Option<U256>,
    /// The balance held by the Settlement contract that is available during a
    /// settlement.
    #[serde_as(as = "HexOrDecimalU256")]
    pub available_balance: U256,
    /// A flag which indicates that solvers are allowed to perform gas cost
    /// optimizations for this token by not routing the trades via an AMM, and
    /// instead use its available balances, as specified by CIP-2.
    pub trusted: bool,
}

impl ToSchema<'static> for TokenInfo {
    fn schema() -> (&'static str, RefOr<Schema>) {
        (
            "TokenInfo",
            Schema::Object(
                ObjectBuilder::new()
                    .description(Some("Information about an ERC20 token."))
                    .required("availableBalance")
                    .required("trusted")
                    .property(
                        "decimals",
                        ObjectBuilder::new()
                            .schema_type(SchemaType::Integer)
                            .description(Some(
                                "The ERC20.decimals value for this token. This may be missing for \
                                 ERC20 tokens that don't implement the optional metadata \
                                 extension.",
                            )),
                    )
                    .property(
                        "symbol",
                        ObjectBuilder::new()
                            .schema_type(SchemaType::String)
                            .description(Some(
                                "The ERC20.symbol value for this token. This may be missing for \
                                 ERC20 tokens that don't implement the optional metadata \
                                 extension.",
                            )),
                    )
                    .property(
                        "referencePrice",
                        AllOfBuilder::new()
                            .description(Some(
                                "The reference price of this token for the auction used for \
                                 scoring. This price is only included for tokens for which there \
                                 are CoW Protocol orders.",
                            ))
                            .item(Ref::from_schema_name("NativePrice")),
                    )
                    .property(
                        "availableBalance",
                        AllOfBuilder::new()
                            .description(Some(
                                "The balance held by the Settlement contract that is available \
                                 during a settlement.",
                            ))
                            .item(Ref::from_schema_name("TokenAmount")),
                    )
                    .property(
                        "trusted",
                        ObjectBuilder::new()
                            .schema_type(SchemaType::Boolean)
                            .description(Some(
                                "A flag which indicates that solvers are allowed to perform gas \
                                 cost optimizations for this token by not routing the trades via \
                                 an AMM, and instead use its available balances, as specified by \
                                 CIP-2.",
                            )),
                    )
                    .build(),
            )
            .into(),
        )
    }
}

#[allow(clippy::enum_variant_names)]
#[derive(Debug, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum Liquidity {
    ConstantProduct(ConstantProductPool),
    WeightedProduct(WeightedProductPool),
    Stable(StablePool),
    ConcentratedLiquidity(ConcentratedLiquidityPool),
    LimitOrder(ForeignLimitOrder),
}

// todo: Currently, it strictly follows the manual api schema. This has to be
// automated and deleted.
impl ToSchema<'static> for Liquidity {
    fn schema() -> (&'static str, RefOr<Schema>) {
        (
            "Liquidity",
            Schema::AllOf(
                AllOfBuilder::new()
                    .description(Some(
                        "On-chain liquidity that can be used in a solution. This liquidity is \
                         provided to facilitate onboarding new solvers. Additional liquidity that \
                         is not included in this set may still be used in solutions.",
                    ))
                    .item(Ref::from_schema_name("LiquidityParameters"))
                    .item(Schema::Object(
                        ObjectBuilder::new()
                            .property(
                                "id",
                                ObjectBuilder::new()
                                    .schema_type(SchemaType::String)
                                    .description(Some(
                                        "An opaque ID used for uniquely identifying the liquidity \
                                         within a single auction (note that they are **not** \
                                         guaranteed to be unique across auctions). This ID is \
                                         used in the solution for matching interactions with the \
                                         executed liquidity.",
                                    )),
                            )
                            .property(
                                "address",
                                AllOfBuilder::new()
                                    .description(Some(
                                        "A rough approximation of gas units required to use this \
                                         liquidity on-chain.",
                                    ))
                                    .item(Ref::from_schema_name("Address")),
                            )
                            .property(
                                "gasEstimate",
                                AllOfBuilder::new()
                                    .description(Some(
                                        "A rough approximation of gas units required to use this \
                                         liquidity on-chain.",
                                    ))
                                    .item(Ref::from_schema_name("BigInt")),
                            )
                            .required("id")
                            .required("address")
                            .required("gasEstimate")
                            .build(),
                    ))
                    .build(),
            )
            .into(),
        )
    }
}

#[allow(clippy::enum_variant_names)]
#[derive(Debug, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase", deny_unknown_fields)]
pub enum LiquidityParameters {
    ConstantProduct(ConstantProductPool),
    WeightedProduct(WeightedProductPool),
    Stable(StablePool),
    ConcentratedLiquidity(ConcentratedLiquidityPool),
    LimitOrder(ForeignLimitOrder),
}

impl ToSchema<'static> for LiquidityParameters {
    fn schema() -> (&'static str, RefOr<Schema>) {
        (
            "LiquidityParameters",
            Schema::OneOf(
                OneOfBuilder::new()
                    .item(Ref::from_schema_name("ConstantProductPool"))
                    .item(Ref::from_schema_name("WeightedProductPool"))
                    .item(Ref::from_schema_name("StablePool"))
                    .item(Ref::from_schema_name("ConcentratedLiquidityPool"))
                    .item(Ref::from_schema_name("ForeignLimitOrder"))
                    .build(),
            )
            .into(),
        )
    }
}

/// A UniswapV2-like constant product liquidity pool for a token pair.
#[serde_as]
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConstantProductPool {
    pub id: String,
    pub address: H160,
    pub router: H160,
    #[serde_as(as = "HexOrDecimalU256")]
    pub gas_estimate: U256,
    pub tokens: HashMap<H160, ConstantProductReserve>,
    pub fee: BigDecimal,
}

impl ToSchema<'static> for ConstantProductPool {
    fn schema() -> (&'static str, RefOr<Schema>) {
        (
            "ConstantProductPool",
            Schema::Object(
                ObjectBuilder::new()
                    .description(Some(
                        "A UniswapV2-like constant product liquidity pool for a token pair.",
                    ))
                    .required("kind")
                    .required("router")
                    .required("tokens")
                    .required("fee")
                    .property(
                        "kind",
                        ObjectBuilder::new()
                            .schema_type(SchemaType::String)
                            .enum_values(Some(["constantProduct"])),
                    )
                    .property("router", Ref::from_schema_name("Address"))
                    .property(
                        "tokens",
                        ObjectBuilder::new()
                            .description(Some("A mapping of token address to its reserve amounts."))
                            .additional_properties(Some(Ref::from_schema_name("TokenReserve"))),
                    )
                    .property("fee", Ref::from_schema_name("Decimal"))
                    .build(),
            )
            .into(),
        )
    }
}

/// A reserve of tokens in an on-chain liquidity pool.
#[serde_as]
#[derive(Debug, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
#[schema(title = "TokenReserve")]
pub struct ConstantProductReserve {
    #[serde_as(as = "HexOrDecimalU256")]
    pub balance: U256,
}

/// A Balancer-like weighted product liquidity pool of N tokens.
#[serde_as]
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WeightedProductPool {
    pub id: String,
    pub address: H160,
    pub balancer_pool_id: H256,
    #[serde_as(as = "HexOrDecimalU256")]
    pub gas_estimate: U256,
    pub tokens: HashMap<H160, WeightedProductReserve>,
    pub fee: BigDecimal,
    pub version: WeightedProductVersion,
}

impl ToSchema<'static> for WeightedProductPool {
    fn schema() -> (&'static str, RefOr<Schema>) {
        (
            "WeightedProductPool",
            Schema::Object(
                ObjectBuilder::new()
                    .description(Some(
                        "A Balancer-like weighted product liquidity pool of N tokens.",
                    ))
                    .required("kind")
                    .required("tokens")
                    .required("fee")
                    .required("balancer_pool_id")
                    .property(
                        "kind",
                        ObjectBuilder::new()
                            .schema_type(SchemaType::String)
                            .enum_values(Some(["weightedProduct"])),
                    )
                    .property(
                        "tokens",
                        ObjectBuilder::new()
                            .description(Some(
                                "A mapping of token address to its reserve amounts with weights.",
                            ))
                            .additional_properties(Some(Schema::AllOf(
                                AllOfBuilder::new()
                                    .item(Ref::from_schema_name("TokenReserve"))
                                    .item(
                                        ObjectBuilder::new()
                                            .required("weight")
                                            .property("weight", Ref::from_schema_name("Decimal"))
                                            .property(
                                                "scalingFactor",
                                                Ref::from_schema_name("Decimal"),
                                            )
                                            .build(),
                                    )
                                    .build(),
                            ))),
                    )
                    .property("fee", Ref::from_schema_name("Decimal"))
                    .property("balancer_pool_id", Ref::from_schema_name("BalancerPoolId"))
                    .property(
                        "version",
                        ObjectBuilder::new()
                            .schema_type(SchemaType::String)
                            .enum_values(Some(["v0", "v3Plus"])),
                    )
                    .build(),
            )
            .into(),
        )
    }
}

#[serde_as]
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WeightedProductReserve {
    #[serde_as(as = "HexOrDecimalU256")]
    pub balance: U256,
    pub scaling_factor: BigDecimal,
    pub weight: BigDecimal,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum WeightedProductVersion {
    V0,
    V3Plus,
}

/// A Curve-like stable pool of N tokens.
#[serde_as]
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StablePool {
    pub id: String,
    pub address: H160,
    pub balancer_pool_id: H256,
    #[serde_as(as = "HexOrDecimalU256")]
    pub gas_estimate: U256,
    pub tokens: HashMap<H160, StableReserve>,
    pub amplification_parameter: BigDecimal,
    pub fee: BigDecimal,
}

impl ToSchema<'static> for StablePool {
    fn schema() -> (&'static str, RefOr<Schema>) {
        (
            "StablePool",
            Schema::Object(
                ObjectBuilder::new()
                    .description(Some("A Curve-like stable pool of N tokens."))
                    .required("kind")
                    .required("tokens")
                    .required("amplificationParameter")
                    .required("fee")
                    .required("balancer_pool_id")
                    .property(
                        "kind",
                        ObjectBuilder::new()
                            .schema_type(SchemaType::String)
                            .enum_values(Some(["stable"])),
                    )
                    .property(
                        "tokens",
                        ObjectBuilder::new()
                            .description(Some(
                                "A mapping of token address to token balance and scaling rate.",
                            ))
                            .additional_properties(Some(Schema::AllOf(
                                AllOfBuilder::new()
                                    .item(Ref::from_schema_name("TokenReserve"))
                                    .item(
                                        ObjectBuilder::new()
                                            .required("scalingFactor")
                                            .property(
                                                "scalingFactor",
                                                Ref::from_schema_name("Decimal"),
                                            )
                                            .build(),
                                    )
                                    .build(),
                            ))),
                    )
                    .property("amplificationParameter", Ref::from_schema_name("Decimal"))
                    .property("fee", Ref::from_schema_name("Decimal"))
                    .property("balancer_pool_id", Ref::from_schema_name("BalancerPoolId"))
                    .build(),
            )
            .into(),
        )
    }
}

#[serde_as]
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StableReserve {
    #[serde_as(as = "HexOrDecimalU256")]
    pub balance: U256,
    pub scaling_factor: BigDecimal,
}

#[serde_as]
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConcentratedLiquidityPool {
    pub id: String,
    pub address: H160,
    pub router: H160,
    #[serde_as(as = "HexOrDecimalU256")]
    pub gas_estimate: U256,
    pub tokens: Vec<H160>,
    #[serde_as(as = "HexOrDecimalU256")]
    pub sqrt_price: U256,
    #[serde_as(as = "DisplayFromStr")]
    pub liquidity: u128,
    pub tick: i32,
    #[serde_as(as = "HashMap<DisplayFromStr, DisplayFromStr>")]
    pub liquidity_net: HashMap<i32, i128>,
    pub fee: BigDecimal,
}

impl ToSchema<'static> for ConcentratedLiquidityPool {
    fn schema() -> (&'static str, RefOr<Schema>) {
        (
            "ConcentratedLiquidityPool",
            Schema::Object(
                ObjectBuilder::new()
                    .description(Some("A Uniswap V3-like concentrated liquidity pool."))
                    .required("kind")
                    .required("router")
                    .required("tokens")
                    .required("sqrtPrice")
                    .required("liquidity")
                    .required("tick")
                    .required("liquidityNet")
                    .required("fee")
                    .property(
                        "kind",
                        ObjectBuilder::new()
                            .schema_type(SchemaType::String)
                            .enum_values(Some(["concentratedLiquidity"])),
                    )
                    .property("router", Ref::from_schema_name("Address"))
                    .property(
                        "tokens",
                        ArrayBuilder::new().items(Ref::from_schema_name("Token")),
                    )
                    .property("sqrtPrice", Ref::from_schema_name("U256"))
                    .property("liquidity", Ref::from_schema_name("U128"))
                    .property("tick", Ref::from_schema_name("I32"))
                    .property(
                        "liquidityNet",
                        ObjectBuilder::new()
                            .description(Some("A map of tick indices to their liquidity values."))
                            .additional_properties(Some(Ref::from_schema_name("I128"))),
                    )
                    .property("fee", Ref::from_schema_name("Decimal"))
                    .build(),
            )
            .into(),
        )
    }
}

#[serde_as]
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ForeignLimitOrder {
    pub id: String,
    pub address: H160,
    #[serde_as(as = "HexOrDecimalU256")]
    pub gas_estimate: U256,
    #[serde_as(as = "serialize::Hex")]
    pub hash: [u8; 32],
    pub maker_token: H160,
    pub taker_token: H160,
    #[serde_as(as = "HexOrDecimalU256")]
    pub maker_amount: U256,
    #[serde_as(as = "HexOrDecimalU256")]
    pub taker_amount: U256,
    #[serde_as(as = "HexOrDecimalU256")]
    pub taker_token_fee_amount: U256,
}

impl ToSchema<'static> for ForeignLimitOrder {
    fn schema() -> (&'static str, RefOr<Schema>) {
        (
            "ForeignLimitOrder",
            Schema::Object(
                ObjectBuilder::new()
                    .description(Some("A 0x-like limit order external to CoW Protocol."))
                    .required("kind")
                    .required("makerToken")
                    .required("takerToken")
                    .required("makerAmount")
                    .required("takerAmount")
                    .required("takerTokenFeeAmount")
                    .property(
                        "kind",
                        ObjectBuilder::new()
                            .schema_type(SchemaType::String)
                            .enum_values(Some(["limitOrder"])),
                    )
                    .property("makerToken", Ref::from_schema_name("Token"))
                    .property("takerToken", Ref::from_schema_name("Token"))
                    .property("makerAmount", Ref::from_schema_name("TokenAmount"))
                    .property("takerAmount", Ref::from_schema_name("TokenAmount"))
                    .property("takerTokenFeeAmount", Ref::from_schema_name("TokenAmount"))
                    .build(),
            )
            .into(),
        )
    }
}

// Structs for the utoipa OpenAPI schema generator.

/// The price in wei of the native token (Ether on Mainnet for example) to buy
/// 10**18 of a token.
#[derive(ToSchema)]
#[schema(example = "1234567890")]
#[allow(dead_code)]
pub struct NativePrice(String);

/// Amount of an ERC20 token. 256 bit unsigned integer in decimal notation.
#[derive(ToSchema)]
#[schema(example = "1234567890")]
#[allow(dead_code)]
pub struct TokenAmount(String);

/// An ISO-8601 formatted date-time.
#[derive(ToSchema)]
#[schema(example = "1970-01-01T00:00:00.000Z")]
#[allow(dead_code)]
pub struct DateTime(String);

/// An Ethereum public address.
#[derive(ToSchema)]
#[schema(example = "0x0000000000000000000000000000000000000000")]
#[allow(dead_code)]
pub struct Address(String);

/// An arbitrary-precision integer value.
#[derive(ToSchema)]
#[schema(example = "1234567890")]
#[allow(dead_code)]
pub struct BigInt(String);

/// An arbitrary-precision decimal value.
#[derive(ToSchema)]
#[schema(example = "13.37")]
#[allow(dead_code)]
pub struct Decimal(String);

/// A hex-encoded 32 byte string containing the pool address (0..20), the pool
/// specialization (20..22) and the poolnonce (22..32).
#[derive(ToSchema)]
#[schema(example = "0xc88c76dd8b92408fe9bea1a54922a31e232d873c0002000000000000000005b2")]
#[allow(dead_code)]
pub struct BalancerPoolId(String);

/// An ERC20 token address.
#[derive(ToSchema)]
#[schema(example = "0xDEf1CA1fb7FBcDC777520aa7f396b4E015F497aB")]
#[allow(dead_code)]
pub struct Token(String);

#[serde_as]
#[derive(ToSchema, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TokenReserve {
    #[schema(value_type = TokenAmount)]
    pub balance: U256,
}

/// 256 bit unsigned integer in decimal notation.
#[derive(ToSchema)]
#[schema(as = U256, example = "1234567890")]
#[allow(dead_code)]
pub struct U256Schema(String);

/// 128 bit unsigned integer in decimal notation.
#[derive(ToSchema)]
#[schema(example = "1234567890")]
#[allow(dead_code)]
pub struct U128(String);

/// 128 bit signed integer in decimal notation.
#[derive(ToSchema)]
#[schema(example = "-1234567890")]
#[allow(dead_code)]
pub struct I128(String);

/// 32 bit signed integer in decimal notation.
#[derive(ToSchema)]
#[schema(example = "-12345")]
#[allow(dead_code)]
pub struct I32(String);

/// Unique identifier for the order. Order UIDs are 56 bytes long, where bytes
/// [0, 32) represent the order digest used for signing, bytes [32, 52)
/// represent the owner address and bytes [52, 56) represent the order's
/// `validTo` field.
#[derive(ToSchema)]
#[schema(
    example = "0x30cff40d9f60caa68a37f0ee73253ad6ad72b45580c945fe3ab67596476937197854163b1b0d24e77dca702b97b5cc33e0f83dcb626122a6"
)]
#[allow(dead_code)]
pub struct OrderUid(String);

/// If the order receives more than limit price, pay the protocol a factor of
/// the difference.
pub struct SurplusFee {
    pub factor: f64,
    pub max_volume_factor: f64,
}

impl ToSchema<'static> for SurplusFee {
    fn schema() -> (&'static str, RefOr<Schema>) {
        (
            "SurplusFee",
            Schema::Object(
                ObjectBuilder::new()
                    .description(Some(
                        "If the order receives more than limit price, pay the protocol a factor \
                         of the difference.",
                    ))
                    .property(
                        "kind",
                        ObjectBuilder::new()
                            .schema_type(SchemaType::String)
                            .enum_values(Some(["surplus"])),
                    )
                    .property(
                        "factor",
                        ObjectBuilder::new()
                            .description(Some(
                                "The factor of the user surplus that the protocol will request \
                                 from the solver after settling the order",
                            ))
                            .schema_type(SchemaType::Number)
                            .example(Number::from_f64(0.5).map(Value::Number))
                            .build(),
                    )
                    .property(
                        "maxVolumeFactor",
                        ObjectBuilder::new()
                            .description(Some(
                                "Never charge more than that percentage of the order volume.",
                            ))
                            .schema_type(SchemaType::Number)
                            .example(Number::from_f64(0.05).map(Value::Number))
                            .minimum(Some(0.0))
                            .maximum(Some(0.99999))
                            .build(),
                    )
                    .build(),
            )
            .into(),
        )
    }
}

/// A cut from the price improvement over the best quote is taken as a protocol
/// fee.
pub struct PriceImprovement {
    pub factor: f64,
    pub max_volume_factor: f64,
    pub quote: Quote,
}

impl ToSchema<'static> for PriceImprovement {
    fn schema() -> (&'static str, RefOr<Schema>) {
        (
            "PriceImprovement",
            Schema::Object(
                ObjectBuilder::new()
                    .description(Some(
                        "A cut from the price improvement over the best quote is taken as a \
                         protocol fee.",
                    ))
                    .property(
                        "kind",
                        ObjectBuilder::new()
                            .schema_type(SchemaType::String)
                            .enum_values(Some(["priceImprovement"])),
                    )
                    .property(
                        "factor",
                        ObjectBuilder::new()
                            .description(Some(
                                "The factor of the user surplus that the protocol will request \
                                 from the solver after settling the order",
                            ))
                            .schema_type(SchemaType::Number)
                            .example(Number::from_f64(0.5).map(Value::Number)),
                    )
                    .property(
                        "maxVolumeFactor",
                        ObjectBuilder::new()
                            .description(Some(
                                "Never charge more than that percentage of the order volume.",
                            ))
                            .schema_type(SchemaType::Number)
                            .example(Number::from_f64(0.01).map(Value::Number))
                            .minimum(Some(0.0))
                            .maximum(Some(0.99999)),
                    )
                    .property("quote", Ref::from_schema_name("Quote"))
                    .build(),
            )
            .into(),
        )
    }
}

pub struct VolumeFee {
    pub factor: f64,
}

impl ToSchema<'static> for VolumeFee {
    fn schema() -> (&'static str, RefOr<Schema>) {
        (
            "VolumeFee",
            Schema::Object(
                ObjectBuilder::new()
                    .property(
                        "kind",
                        ObjectBuilder::new()
                            .schema_type(SchemaType::String)
                            .enum_values(Some(["volume"])),
                    )
                    .property(
                        "factor",
                        ObjectBuilder::new()
                            .description(Some(
                                "The fraction of the order's volume that the protocol will \
                                 request from the solver after settling the order.",
                            ))
                            .schema_type(SchemaType::Number)
                            .example(Number::from_f64(0.5).map(Value::Number)),
                    )
                    .build(),
            )
            .into(),
        )
    }
}
