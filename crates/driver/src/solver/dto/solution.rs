use {
    crate::{logic::competition::solution, util::serialize},
    ethereum_types::H160,
    serde::Deserialize,
    std::collections::HashMap,
};

impl From<Solution> for solution::Solution {
    fn from(_solution: Solution) -> Self {
        todo!()
    }
}

#[derive(Debug, Deserialize)]
pub struct Solution {
    orders: HashMap<usize, Order>,
    #[serde(default)]
    foreign_liquidity_orders: Vec<ForeignLiquidityOrder>,
    #[serde(default)]
    amms: HashMap<H160, Amm>,
    ref_token: Option<H160>,
    prices: HashMap<H160, serialize::U256>,
    #[serde(default)]
    approvals: Vec<Approval>,
    #[serde(default)]
    interaction_data: Vec<Interaction>,
    metadata: Option<Metadata>,
}

#[derive(Debug, Deserialize)]
struct Order {
    exec_sell_amount: serialize::U256,
    exec_buy_amount: serialize::U256,
    cost: Option<TokenAmount>,
    fee: Option<TokenAmount>,
    exec_plan: Option<ExecutionPlan>,
}

#[derive(Debug, Deserialize)]
struct TokenAmount {
    amount: serialize::U256,
    token: H160,
}

#[derive(Debug, Deserialize)]
struct ForeignLiquidityOrder {
    order: LiquidityOrder,
    exec_sell_amount: serialize::U256,
    exec_buy_amount: serialize::U256,
}

#[derive(Debug, Deserialize)]
struct Amm {
    order: LiquidityOrder,
    exec_sell_amount: serialize::U256,
    exec_buy_amount: serialize::U256,
}

#[derive(Debug, Deserialize)]
struct LiquidityOrder {
    from: H160,
    sell_token: H160,
    buy_token: H160,
    #[serde(default)]
    receiver: Option<H160>,
    sell_amount: serialize::U256,
    buy_amount: serialize::U256,
    valid_to: u32,
    app_data: [u8; 32],
    fee_amount: serialize::U256,
    kind: OrderKind,
    partially_fillable: bool,
    #[serde(default)]
    sell_token_balance: SellTokenSource,
    #[serde(default)]
    buy_token_balance: BuyTokenDestination,
    #[serde(flatten)]
    signature: Signature,
}

#[derive(Debug, Deserialize)]
struct Approval {
    token: H160,
    spender: H160,
    amount: serialize::U256,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "lowercase")]
enum OrderKind {
    Buy,
    Sell,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "snake_case")]
enum SellTokenSource {
    #[default]
    Erc20,
    Internal,
    External,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "snake_case")]
enum BuyTokenDestination {
    #[default]
    Erc20,
    Internal,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Interaction {
    target: H160,
    value: serialize::U256,
    call_data: Vec<u8>,
}

#[derive(Debug, Deserialize)]
struct Metadata {
    has_solution: Option<bool>,
    result: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum ExecutionPlan {
    Coordinates(Coordinates),
    #[serde(deserialize_with = "execution_plan_internal")]
    Internal,
}

#[derive(Debug, Deserialize)]
struct Coordinates {
    sequence: u32,
    position: u32,
    internal: bool,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Signature {
    signing_scheme: SigningScheme,
    signature: serialize::Hex,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "lowercase")]
enum SigningScheme {
    Eip712,
    EthSign,
    Eip1271,
    PreSign,
}

/// Work-around for untagged enum serialization not supporting empty variants.
///
/// https://github.com/serde-rs/serde/issues/1560
fn execution_plan_internal<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> Result<(), D::Error> {
    #[derive(Deserialize)]
    enum Kind {
        #[serde(rename = "internal")]
        Internal,
    }

    Kind::deserialize(deserializer)?;
    Ok(())
}
