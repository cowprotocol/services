use {
    crate::{
        domain::{order, solution},
        util::serialize,
    },
    ethereum_types::{H160, U256},
    serde::Serialize,
    serde_with::serde_as,
    std::collections::HashMap,
};

impl Solutions {
    /// Creates a new solution DTO from its domain object.
    pub fn from_domain(solutions: &[solution::Solution]) -> Self {
        Self {
            solutions: solutions
                .iter()
                .map(|solution| Solution {
                    id: solution.id.0,
                    prices: solution
                        .prices
                        .0
                        .iter()
                        .map(|(token, price)| (token.0, *price))
                        .collect(),
                    trades: solution
                        .trades
                        .iter()
                        .map(|trade| match trade {
                            solution::Trade::Fulfillment(trade) => {
                                Trade::Fulfillment(Fulfillment {
                                    order: trade.order().uid.0,
                                    executed_amount: trade.executed().amount,
                                    fee: trade.surplus_fee().map(|fee| fee.amount),
                                })
                            }
                            solution::Trade::Jit(trade) => {
                                let (signing_scheme, signature) = match &trade.order.signature {
                                    order::Signature::Eip712(signature) => {
                                        (SigningScheme::Eip712, signature.to_bytes().to_vec())
                                    }
                                    order::Signature::EthSign(signature) => {
                                        (SigningScheme::EthSign, signature.to_bytes().to_vec())
                                    }
                                    order::Signature::Eip1271(bytes) => {
                                        (SigningScheme::Eip1271, bytes.clone())
                                    }
                                    order::Signature::PreSign => (SigningScheme::PreSign, vec![]),
                                };

                                Trade::Jit(JitTrade {
                                    order: JitOrder {
                                        sell_token: trade.order.sell.token.0,
                                        sell_amount: trade.order.sell.amount,
                                        buy_token: trade.order.buy.token.0,
                                        buy_amount: trade.order.buy.amount,
                                        receiver: trade.order.receiver,
                                        valid_to: trade.order.valid_to,
                                        app_data: trade.order.app_data.0,
                                        fee_amount: trade.order.fee.0,
                                        kind: match trade.order.side {
                                            crate::domain::order::Side::Buy => Kind::Buy,
                                            crate::domain::order::Side::Sell => Kind::Sell,
                                        },
                                        partially_fillable: trade.order.partially_fillable,
                                        sell_token_balance: SellTokenBalance::Erc20,
                                        buy_token_balance: BuyTokenBalance::Erc20,
                                        signing_scheme,
                                        signature,
                                    },
                                    executed_amount: trade.executed,
                                })
                            }
                        })
                        .collect(),
                    interactions: solution
                        .interactions
                        .iter()
                        .map(|interaction| match interaction {
                            solution::Interaction::Liquidity(interaction) => {
                                Interaction::Liquidity(LiquidityInteraction {
                                    id: interaction.liquidity.id.0.clone(),
                                    input_token: interaction.input.token.0,
                                    input_amount: interaction.input.amount,
                                    output_token: interaction.output.token.0,
                                    output_amount: interaction.output.amount,
                                    internalize: interaction.internalize,
                                })
                            }
                            solution::Interaction::Custom(interaction) => {
                                Interaction::Custom(CustomInteraction {
                                    target: interaction.target,
                                    value: interaction.value.0,
                                    calldata: interaction.calldata.clone(),
                                    internalize: interaction.internalize,
                                    allowances: interaction
                                        .allowances
                                        .iter()
                                        .map(|allowance| Allowance {
                                            token: allowance.asset.token.0,
                                            amount: allowance.asset.amount,
                                            spender: allowance.spender,
                                        })
                                        .collect(),
                                    inputs: interaction
                                        .inputs
                                        .iter()
                                        .map(|i| Asset {
                                            token: i.token.0,
                                            amount: i.amount,
                                        })
                                        .collect(),
                                    outputs: interaction
                                        .outputs
                                        .iter()
                                        .map(|o| Asset {
                                            token: o.token.0,
                                            amount: o.amount,
                                        })
                                        .collect(),
                                })
                            }
                        })
                        .collect(),
                    score: match solution.score.clone() {
                        solution::Score::Solver(score) => Score::Solver { score },
                        solution::Score::RiskAdjusted(score) => Score::RiskAdjusted {
                            success_probability: score.0,
                        },
                        solution::Score::Surplus => Score::Surplus,
                    },
                })
                .collect(),
        }
    }
}

#[derive(Debug, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Solutions {
    solutions: Vec<Solution>,
}

#[serde_as]
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct Solution {
    id: u64,
    #[serde_as(as = "HashMap<_, serialize::U256>")]
    prices: HashMap<H160, U256>,
    trades: Vec<Trade>,
    interactions: Vec<Interaction>,
    score: Score,
}

#[derive(Debug, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
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
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde_as(as = "Option<serialize::U256>")]
    fee: Option<U256>,
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
#[serde(rename_all = "camelCase")]
enum Kind {
    Sell,
    Buy,
}

#[derive(Debug, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
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
    #[serde(rename = "callData")]
    #[serde_as(as = "serialize::Hex")]
    calldata: Vec<u8>,
    allowances: Vec<Allowance>,
    inputs: Vec<Asset>,
    outputs: Vec<Asset>,
}

/// An interaction that can be executed as part of an order's pre- or
/// post-interactions.
#[serde_as]
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct OrderInteraction {
    target: H160,
    #[serde_as(as = "serialize::U256")]
    value: U256,
    #[serde(rename = "callData")]
    #[serde_as(as = "serialize::Hex")]
    calldata: Vec<u8>,
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
#[serde(rename_all = "camelCase")]
enum SellTokenBalance {
    #[default]
    Erc20,
    Internal,
    External,
}

#[derive(Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
enum BuyTokenBalance {
    #[default]
    Erc20,
    Internal,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
enum SigningScheme {
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
        #[serde_as(as = "serialize::U256")]
        score: U256,
    },
    #[serde(rename_all = "camelCase")]
    RiskAdjusted {
        success_probability: f64,
    },
    Surplus,
}
