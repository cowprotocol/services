use {
    crate::{logic, util::serialize},
    ethereum_types::{H160, U256},
    serde::Deserialize,
    serde_with::serde_as,
    std::collections::HashMap,
};

impl From<Solution> for logic::competition::Solution {
    fn from(_solution: Solution) -> Self {
        todo!()
    }
}

#[serde_as]
#[derive(Debug, Deserialize)]
pub struct Solution {
    orders: HashMap<usize, Order>,
    #[serde(default)]
    foreign_liquidity_orders: Vec<ForeignLiquidityOrder>,
    #[serde(default)]
    amms: HashMap<H160, Amm>,
    #[serde_as(as = "HashMap<_, serialize::U256>")]
    prices: HashMap<H160, U256>,
    #[serde(default)]
    approvals: Vec<Approval>,
    #[serde(default)]
    interaction_data: Vec<Interaction>,
    // TODO What is this?
    metadata: Option<Metadata>,
}

#[serde_as]
#[derive(Debug, Deserialize)]
struct Order {
    #[serde_as(as = "serialize::U256")]
    exec_sell_amount: U256,
    #[serde_as(as = "serialize::U256")]
    exec_buy_amount: U256,
    cost: Option<TokenAmount>,
    fee: Option<TokenAmount>,
    // TODO: #831 should get rid of this
    exec_plan: Option<ExecutionPlan>,
}

#[serde_as]
#[derive(Debug, Deserialize)]
struct TokenAmount {
    #[serde_as(as = "serialize::U256")]
    amount: U256,
    token: H160,
}

#[serde_as]
#[derive(Debug, Deserialize)]
struct ForeignLiquidityOrder {
    order: LiquidityOrder,
    #[serde_as(as = "serialize::U256")]
    exec_sell_amount: U256,
    #[serde_as(as = "serialize::U256")]
    exec_buy_amount: U256,
}

#[serde_as]
#[derive(Debug, Deserialize)]
struct Amm {
    execution: Vec<AmmExecution>,
}

// TODO Will be fixed after #831
#[serde_as]
#[derive(Debug, Deserialize)]
struct AmmExecution {
    sell_token: H160,
    buy_token: H160,
    #[serde_as(as = "serialize::U256")]
    exec_sell_amount: U256,
    #[serde_as(as = "serialize::U256")]
    exec_buy_amount: U256,
    exec_plan: ExecutionPlan,
}

#[serde_as]
#[derive(Debug, Deserialize)]
struct LiquidityOrder {
    from: H160,
    sell_token: H160,
    buy_token: H160,
    #[serde(default)]
    receiver: Option<H160>,
    #[serde_as(as = "serialize::U256")]
    sell_amount: U256,
    #[serde_as(as = "serialize::U256")]
    buy_amount: U256,
    valid_to: u32,
    app_data: [u8; 32],
    #[serde_as(as = "serialize::U256")]
    fee_amount: U256,
    kind: OrderKind,
    partially_fillable: bool,

    #[serde(default)]
    sell_token_balance: SellTokenSource,
    #[serde(default)]
    buy_token_balance: BuyTokenDestination,

    #[serde(flatten)]
    signature: Signature,
}

#[serde_as]
#[derive(Debug, Deserialize)]
struct Approval {
    token: H160,
    spender: H160,
    #[serde_as(as = "serialize::U256")]
    amount: U256,
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

#[serde_as]
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Interaction {
    target: H160,
    #[serde_as(as = "serialize::U256")]
    value: U256,
    call_data: Vec<u8>,
    inputs: Vec<TokenAmount>,
    outputs: Vec<TokenAmount>,
}

#[derive(Debug, Deserialize)]
struct Metadata {
    has_solution: Option<bool>,
    result: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ExecutionPlan {
    sequence: u32,
    position: u32,
    internal: bool,
}

#[serde_as]
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Signature {
    signing_scheme: SigningScheme,
    #[serde_as(as = "serialize::Hex")]
    signature: Vec<u8>,
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
