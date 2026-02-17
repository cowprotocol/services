use {
    alloy::primitives::{Address, B256, U256},
    app_data::AppDataHash,
    bigdecimal::BigDecimal,
    number::serialization::HexOrDecimalU256,
    serde::{Deserialize, Serialize},
    serde_with::{DisplayFromStr, serde_as},
    std::collections::HashMap,
};

#[serde_as]
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Auction {
    #[serde_as(as = "Option<DisplayFromStr>")]
    pub id: Option<i64>,
    pub tokens: HashMap<Address, Token>,
    pub orders: Vec<Order>,
    pub liquidity: Vec<Liquidity>,
    #[serde_as(as = "HexOrDecimalU256")]
    pub effective_gas_price: U256,
    pub deadline: chrono::DateTime<chrono::Utc>,
    pub surplus_capturing_jit_order_owners: Vec<Address>,
}

#[serde_as]
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Order {
    #[serde_as(as = "serde_ext::Hex")]
    pub uid: [u8; 56],
    pub sell_token: Address,
    pub buy_token: Address,
    #[serde_as(as = "HexOrDecimalU256")]
    pub sell_amount: U256,
    #[serde_as(as = "HexOrDecimalU256")]
    pub full_sell_amount: U256,
    #[serde_as(as = "HexOrDecimalU256")]
    pub buy_amount: U256,
    #[serde_as(as = "HexOrDecimalU256")]
    pub full_buy_amount: U256,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fee_policies: Option<Vec<FeePolicy>>,
    pub valid_to: u32,
    pub kind: Kind,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub receiver: Option<Address>,
    pub owner: Address,
    pub partially_fillable: bool,
    pub pre_interactions: Vec<InteractionData>,
    pub post_interactions: Vec<InteractionData>,
    pub sell_token_source: SellTokenSource,
    pub buy_token_destination: BuyTokenDestination,
    pub class: Class,
    pub app_data: AppDataHash,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub flashloan_hint: Option<FlashloanHint>,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub wrappers: Vec<WrapperCall>,
    pub signing_scheme: SigningScheme,
    #[serde(with = "bytes_hex")]
    pub signature: Vec<u8>,
}

/// Destination for which the buyAmount should be transferred to order's
/// receiver to upon fulfillment
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BuyTokenDestination {
    /// Pay trade proceeds as an ERC20 token transfer
    Erc20,
    /// Pay trade proceeds as a Vault internal balance transfer
    Internal,
}

/// Source from which the sellAmount should be drawn upon order fulfillment
#[derive(Debug, Serialize, Deserialize)]
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
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InteractionData {
    pub target: Address,
    #[serde_as(as = "HexOrDecimalU256")]
    pub value: U256,
    #[serde(with = "bytes_hex")]
    pub call_data: Vec<u8>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SigningScheme {
    Eip712,
    EthSign,
    Eip1271,
    PreSign,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Kind {
    Sell,
    Buy,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Class {
    Market,
    Limit,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum FeePolicy {
    #[serde(rename_all = "camelCase")]
    Surplus { factor: f64, max_volume_factor: f64 },
    #[serde(rename_all = "camelCase")]
    PriceImprovement {
        factor: f64,
        max_volume_factor: f64,
        quote: Quote,
    },
    #[serde(rename_all = "camelCase")]
    Volume { factor: f64 },
}

#[serde_as]
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Quote {
    #[serde_as(as = "HexOrDecimalU256")]
    pub sell_amount: U256,
    #[serde_as(as = "HexOrDecimalU256")]
    pub buy_amount: U256,
    #[serde_as(as = "HexOrDecimalU256")]
    pub fee: U256,
}

#[serde_as]
#[derive(Debug, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Token {
    pub decimals: Option<u8>,
    pub symbol: Option<String>,
    #[serde_as(as = "Option<HexOrDecimalU256>")]
    pub reference_price: Option<U256>,
    #[serde_as(as = "HexOrDecimalU256")]
    pub available_balance: U256,
    pub trusted: bool,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum Liquidity {
    ConstantProduct(ConstantProductPool),
    WeightedProduct(WeightedProductPool),
    Stable(StablePool),
    ConcentratedLiquidity(ConcentratedLiquidityPool),
    LimitOrder(ForeignLimitOrder),
}

#[serde_as]
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConstantProductPool {
    pub id: String,
    pub address: Address,
    pub router: Address,
    #[serde_as(as = "HexOrDecimalU256")]
    pub gas_estimate: U256,
    pub tokens: HashMap<Address, ConstantProductReserve>,
    pub fee: BigDecimal,
}

#[serde_as]
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConstantProductReserve {
    #[serde_as(as = "HexOrDecimalU256")]
    pub balance: U256,
}

#[serde_as]
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WeightedProductPool {
    pub id: String,
    pub address: Address,
    pub balancer_pool_id: B256,
    #[serde_as(as = "HexOrDecimalU256")]
    pub gas_estimate: U256,
    pub tokens: HashMap<Address, WeightedProductReserve>,
    pub fee: BigDecimal,
    pub version: WeightedProductVersion,
}

#[serde_as]
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WeightedProductReserve {
    #[serde_as(as = "HexOrDecimalU256")]
    pub balance: U256,
    pub scaling_factor: BigDecimal,
    pub weight: BigDecimal,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum WeightedProductVersion {
    V0,
    V3Plus,
}

#[serde_as]
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StablePool {
    pub id: String,
    pub address: Address,
    pub balancer_pool_id: B256,
    #[serde_as(as = "HexOrDecimalU256")]
    pub gas_estimate: U256,
    pub tokens: HashMap<Address, StableReserve>,
    pub amplification_parameter: BigDecimal,
    pub fee: BigDecimal,
}

#[serde_as]
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StableReserve {
    #[serde_as(as = "HexOrDecimalU256")]
    pub balance: U256,
    pub scaling_factor: BigDecimal,
}

#[serde_as]
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConcentratedLiquidityPool {
    pub id: String,
    pub address: Address,
    pub router: Address,
    #[serde_as(as = "HexOrDecimalU256")]
    pub gas_estimate: U256,
    pub tokens: Vec<Address>,
    #[serde_as(as = "HexOrDecimalU256")]
    pub sqrt_price: U256,
    #[serde_as(as = "DisplayFromStr")]
    pub liquidity: u128,
    pub tick: i32,
    #[serde_as(as = "HashMap<DisplayFromStr, DisplayFromStr>")]
    pub liquidity_net: HashMap<i32, i128>,
    pub fee: BigDecimal,
}

#[serde_as]
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ForeignLimitOrder {
    pub id: String,
    pub address: Address,
    #[serde_as(as = "HexOrDecimalU256")]
    pub gas_estimate: U256,
    #[serde_as(as = "serde_ext::Hex")]
    pub hash: [u8; 32],
    pub maker_token: Address,
    pub taker_token: Address,
    #[serde_as(as = "HexOrDecimalU256")]
    pub maker_amount: U256,
    #[serde_as(as = "HexOrDecimalU256")]
    pub taker_amount: U256,
    #[serde_as(as = "HexOrDecimalU256")]
    pub taker_token_fee_amount: U256,
}

#[serde_as]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FlashloanHint {
    pub liquidity_provider: Address,
    pub protocol_adapter: Address,
    pub receiver: Address,
    pub token: Address,
    #[serde_as(as = "HexOrDecimalU256")]
    pub amount: U256,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WrapperCall {
    pub address: Address,
    #[serde(with = "bytes_hex")]
    pub data: Vec<u8>,
    /// Declares whether this wrapper (and its data) needs to be included
    /// unmodified in a solution containing this order.
    #[serde(default)]
    pub is_omittable: bool,
}
