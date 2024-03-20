use {
    super::serialize,
    number::serialization::HexOrDecimalU256,
    serde::Serialize,
    serde_with::serde_as,
    std::collections::HashMap,
    web3::types::{H160, U256},
};

#[derive(Debug, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Solutions {
    pub solutions: Vec<Solution>,
}

#[serde_as]
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Solution {
    pub id: u64,
    #[serde_as(as = "HashMap<_, HexOrDecimalU256>")]
    pub prices: HashMap<H160, U256>,
    pub trades: Vec<Trade>,
    pub interactions: Vec<Interaction>,
    pub score: Score,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gas: Option<u64>,
}

#[derive(Debug, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum Trade {
    Fulfillment(Fulfillment),
    Jit(JitTrade),
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

#[serde_as]
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct JitTrade {
    pub order: JitOrder,
    #[serde_as(as = "HexOrDecimalU256")]
    pub executed_amount: U256,
}

#[serde_as]
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct JitOrder {
    pub sell_token: H160,
    pub buy_token: H160,
    pub receiver: H160,
    #[serde_as(as = "HexOrDecimalU256")]
    pub sell_amount: U256,
    #[serde_as(as = "HexOrDecimalU256")]
    pub buy_amount: U256,
    pub valid_to: u32,
    #[serde_as(as = "serialize::Hex")]
    pub app_data: [u8; 32],
    #[serde_as(as = "HexOrDecimalU256")]
    pub fee_amount: U256,
    pub kind: Kind,
    pub partially_fillable: bool,
    pub sell_token_balance: SellTokenBalance,
    pub buy_token_balance: BuyTokenBalance,
    pub signing_scheme: SigningScheme,
    #[serde_as(as = "serialize::Hex")]
    pub signature: Vec<u8>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Kind {
    Sell,
    Buy,
}

#[derive(Debug, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum Interaction {
    Liquidity(LiquidityInteraction),
    Custom(CustomInteraction),
}

#[serde_as]
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LiquidityInteraction {
    pub internalize: bool,
    pub id: String,
    pub input_token: H160,
    pub output_token: H160,
    #[serde_as(as = "HexOrDecimalU256")]
    pub input_amount: U256,
    #[serde_as(as = "HexOrDecimalU256")]
    pub output_amount: U256,
}

#[serde_as]
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CustomInteraction {
    pub internalize: bool,
    pub target: H160,
    #[serde_as(as = "HexOrDecimalU256")]
    pub value: U256,
    #[serde(rename = "callData")]
    #[serde_as(as = "serialize::Hex")]
    pub calldata: Vec<u8>,
    pub allowances: Vec<Allowance>,
    pub inputs: Vec<Asset>,
    pub outputs: Vec<Asset>,
}

/// An interaction that can be executed as part of an order's pre- or
/// post-interactions.
#[serde_as]
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OrderInteraction {
    pub target: H160,
    #[serde_as(as = "HexOrDecimalU256")]
    pub value: U256,
    #[serde(rename = "callData")]
    #[serde_as(as = "serialize::Hex")]
    pub calldata: Vec<u8>,
}

#[serde_as]
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Asset {
    pub token: H160,
    #[serde_as(as = "HexOrDecimalU256")]
    pub amount: U256,
}

#[serde_as]
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Allowance {
    pub token: H160,
    pub spender: H160,
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

#[derive(Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum BuyTokenBalance {
    #[default]
    Erc20,
    Internal,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum SigningScheme {
    Eip712,
    EthSign,
    PreSign,
    Eip1271,
}

/// A score for a solution. The score is used to rank solutions.
#[serde_as]
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase", tag = "kind")]
pub enum Score {
    Solver {
        #[serde_as(as = "HexOrDecimalU256")]
        score: U256,
    },
    #[serde(rename_all = "camelCase")]
    RiskAdjusted { success_probability: f64 },
}
