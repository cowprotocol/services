use {
    crate::{logic, util::serialize},
    ethereum_types::{H160, U256},
    num::BigUint,
    serde::Deserialize,
    serde_with::{serde_as, DisplayFromStr},
    std::collections::HashMap,
};

impl From<Auction> for logic::competition::Auction {
    fn from(_: Auction) -> Self {
        todo!()
    }
}

#[serde_as]
#[derive(Debug, Deserialize)]
pub struct Auction {
    id: u64,
    block: u64,
    orders: Vec<Order>,
    deadline: chrono::DateTime<chrono::Utc>,
    #[serde_as(as = "HashMap<_, DisplayFromStr>")]
    prices: HashMap<H160, BigUint>,
}

// TODO Should we rename kind to side and class to kind in this interface?
#[serde_as]
#[derive(Debug, Deserialize)]
struct Order {
    #[serde_as(as = "serialize::Hex")]
    uid: [u8; 56],
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
    #[serde_as(as = "serialize::Hex")]
    app_data: [u8; 32],
    #[serde_as(as = "serialize::U256")]
    fee_amount: U256,
    kind: OrderKind,
    partially_fillable: bool,
    #[serde(default)]
    sell_token_balance: SellTokenBalance,
    #[serde(default)]
    buy_token_balance: BuyTokenBalance,
    #[serde_as(as = "serialize::U256")]
    #[serde(default)]
    full_fee_amount: U256,
    #[serde(flatten)]
    class: OrderClass,
    #[serde_as(as = "serialize::U256")]
    executed_amount: U256,
    #[serde(default)]
    interactions: Interactions,
    #[serde(flatten)]
    signature: Signature,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "lowercase")]
enum OrderKind {
    Buy,
    Sell,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "snake_case")]
enum SellTokenBalance {
    #[default]
    Erc20,
    Internal,
    External,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "snake_case")]
enum BuyTokenBalance {
    #[default]
    Erc20,
    Internal,
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

#[derive(Debug, Deserialize)]
#[serde(tag = "class", rename_all = "lowercase")]
enum OrderClass {
    Market,
    Liquidity,
    Limit(LimitOrder),
}

#[serde_as]
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct LimitOrder {
    #[serde_as(as = "serialize::U256")]
    surplus_fee: U256,
}

#[derive(Debug, Default, Deserialize)]
struct Interactions {
    pre: Vec<InteractionData>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
struct InteractionData {
    target: H160,
    value: U256,
    call_data: Vec<u8>,
}
