use {
    crate::domain::{order, solution},
    solvers_dto::solution::*,
};

/// Creates a new solution DTO from its domain object.
pub fn from_domain(solutions: &[solution::Solution]) -> super::Solutions {
    super::Solutions {
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
                        solution::Trade::Fulfillment(trade) => Trade::Fulfillment(Fulfillment {
                            order: trade.order().uid.0,
                            executed_amount: trade.executed().amount,
                            fee: trade.surplus_fee().map(|fee| fee.amount),
                        }),
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
                },
                gas: solution.gas.map(|gas| gas.0.as_u64()),
            })
            .collect(),
    }
}
