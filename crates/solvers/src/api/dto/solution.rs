use {
    crate::{domain::solution, util::serialize},
    ethereum_types::{H160, U256},
    serde::Serialize,
    serde_with::serde_as,
    std::collections::HashMap,
};

impl Solution {
    /// Returns the trivial solution.
    pub fn trivial() -> Self {
        Self {
            prices: Default::default(),
            trades: Default::default(),
            interactions: Default::default(),
        }
    }

    /// Creates a new solution DTO from its domain object.
    pub fn from_domain(solution: &solution::Solution) -> Self {
        Self {
            prices: solution
                .prices
                .0
                .iter()
                .map(|(token, price)| (token.0, *price))
                .collect(),
            trades: solution
                .trades
                .iter()
                .map(|trade| {
                    Trade::Fulfillment(Fulfillment {
                        order: trade.order().uid.0,
                        executed_amount: trade.executed().amount,
                    })
                })
                .collect(),
            interactions: solution
                .interactions
                .iter()
                .map(|interaction| {
                    Interaction::Liquidity(LiquidityInteraction {
                        internalize: false,
                        id: interaction.liquidity.id.0.clone(),
                        input_token: interaction.input.token.0,
                        output_token: interaction.output.token.0,
                        input_amount: interaction.input.amount,
                        output_amount: interaction.output.amount,
                    })
                })
                .collect(),
        }
    }
}

#[serde_as]
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Solution {
    #[serde_as(as = "HashMap<_, serialize::U256>")]
    prices: HashMap<H160, U256>,
    trades: Vec<Trade>,
    interactions: Vec<Interaction>,
}

#[derive(Debug, Serialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
enum Trade {
    Fulfillment(Fulfillment),
    Jit(JitTrade),
}

#[serde_as]
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct Fulfillment {
    #[serde_as(as = "serialize::Hex")]
    order: [u8; 56],
    #[serde_as(as = "serialize::U256")]
    executed_amount: U256,
}

#[serde_as]
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct JitTrade {
    order: JitOrder,
    #[serde_as(as = "serialize::U256")]
    executed_amount: U256,
}

#[serde_as]
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct JitOrder {
    sell_token: H160,
    buy_token: H160,
    receiver: H160,
    #[serde_as(as = "serialize::U256")]
    sell_amount: U256,
    #[serde_as(as = "serialize::U256")]
    buy_amount: U256,
    valid_to: u32,
    #[serde_as(as = "serialize::Hex")]
    app_data: [u8; 32],
    #[serde_as(as = "serialize::U256")]
    fee_amount: U256,
    kind: Kind,
    partially_fillable: bool,
    sell_token_balance: SellTokenBalance,
    buy_token_balance: BuyTokenBalance,
    signing_scheme: SigningScheme,
    #[serde_as(as = "serialize::Hex")]
    signature: Vec<u8>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "lowercase")]
enum Kind {
    Sell,
    Buy,
}

#[derive(Debug, Serialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
enum Interaction {
    Liquidity(LiquidityInteraction),
    Custom(CustomInteraction),
}

#[serde_as]
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct LiquidityInteraction {
    internalize: bool,
    id: String,
    input_token: H160,
    output_token: H160,
    #[serde_as(as = "serialize::U256")]
    input_amount: U256,
    #[serde_as(as = "serialize::U256")]
    output_amount: U256,
}

#[serde_as]
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct CustomInteraction {
    internalize: bool,
    target: H160,
    #[serde_as(as = "serialize::U256")]
    value: U256,
    #[serde_as(as = "serialize::Hex")]
    call_data: Vec<u8>,
    allowances: Vec<Allowance>,
    inputs: Vec<Asset>,
    outputs: Vec<Asset>,
}

#[serde_as]
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct Asset {
    token: H160,
    #[serde_as(as = "serialize::U256")]
    amount: U256,
}

#[serde_as]
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct Allowance {
    token: H160,
    spender: H160,
    #[serde_as(as = "serialize::U256")]
    amount: U256,
}

#[derive(Debug, Default, Serialize)]
#[serde(rename_all = "lowercase")]
enum SellTokenBalance {
    #[default]
    Erc20,
    Internal,
    External,
}

#[derive(Debug, Default, Serialize)]
#[serde(rename_all = "lowercase")]
enum BuyTokenBalance {
    #[default]
    Erc20,
    Internal,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "lowercase")]
enum SigningScheme {
    Eip712,
    EthSign,
    PreSign,
    Eip1271,
}
