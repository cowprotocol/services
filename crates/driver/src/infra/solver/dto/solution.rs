use {
    crate::{
        domain::{competition, competition::order, eth, liquidity},
        infra::Solver,
        util::serialize,
    },
    itertools::Itertools,
    serde::Deserialize,
    serde_with::serde_as,
    std::collections::HashMap,
};

impl Solutions {
    pub fn into_domain(
        self,
        auction: &competition::Auction,
        liquidity: &[liquidity::Liquidity],
        weth: eth::WethAddress,
        solver: Solver,
    ) -> Result<Vec<competition::Solution>, super::Error> {
        self.solutions
            .into_iter()
            .map(|solution| {
                competition::Solution::new(
                    solution.id.into(),
                    solution
                        .trades
                        .into_iter()
                        .map(|trade| match trade {
                            Trade::Fulfillment(fulfillment) => {
                                let order = auction
                                    .orders()
                                    .iter()
                                    .find(|order| order.uid == fulfillment.order)
                                    // TODO this error should reference the UID
                                    .ok_or(super::Error(
                                        "invalid order UID specified in fulfillment".to_owned()
                                    ))?
                                    .clone();

                                competition::solution::trade::Fulfillment::new(
                                    order,
                                    fulfillment.executed_amount.into(),
                                    match fulfillment.fee {
                                        Some(fee) => competition::solution::trade::Fee::Dynamic(
                                            competition::order::SellAmount(fee),
                                        ),
                                        None => competition::solution::trade::Fee::Static,
                                    },
                                )
                                .map(competition::solution::Trade::Fulfillment)
                                .map_err(|err| super::Error(format!("invalid fulfillment: {err}")))
                            }
                            Trade::Jit(jit) => Ok(competition::solution::Trade::Jit(
                                competition::solution::trade::Jit::new(
                                    competition::order::Jit {
                                        sell: eth::Asset {
                                            amount: jit.order.sell_amount.into(),
                                            token: jit.order.sell_token.into(),
                                        },
                                        buy: eth::Asset {
                                            amount: jit.order.buy_amount.into(),
                                            token: jit.order.buy_token.into(),
                                        },
                                        fee: jit.order.fee_amount.into(),
                                        receiver: jit.order.receiver.into(),
                                        valid_to: jit.order.valid_to.into(),
                                        app_data: jit.order.app_data.into(),
                                        side: match jit.order.kind {
                                            Kind::Sell => competition::order::Side::Sell,
                                            Kind::Buy => competition::order::Side::Buy,
                                        },
                                        partially_fillable: jit.order.partially_fillable,
                                        sell_token_balance: match jit.order.sell_token_balance {
                                            SellTokenBalance::Erc20 => {
                                                competition::order::SellTokenBalance::Erc20
                                            }
                                            SellTokenBalance::Internal => {
                                                competition::order::SellTokenBalance::Internal
                                            }
                                            SellTokenBalance::External => {
                                                competition::order::SellTokenBalance::External
                                            }
                                        },
                                        buy_token_balance: match jit.order.buy_token_balance {
                                            BuyTokenBalance::Erc20 => {
                                                competition::order::BuyTokenBalance::Erc20
                                            }
                                            BuyTokenBalance::Internal => {
                                                competition::order::BuyTokenBalance::Internal
                                            }
                                        },
                                        signature: competition::order::Signature {
                                            scheme: match jit.order.signing_scheme {
                                                SigningScheme::Eip712 => {
                                                    competition::order::signature::Scheme::Eip712
                                                }
                                                SigningScheme::EthSign => {
                                                    competition::order::signature::Scheme::EthSign
                                                }
                                                SigningScheme::PreSign => {
                                                    competition::order::signature::Scheme::PreSign
                                                }
                                                SigningScheme::Eip1271 => {
                                                    competition::order::signature::Scheme::Eip1271
                                                }
                                            },
                                            data: jit.order.signature.into(),
                                            signer: solver.address(),
                                        },
                                    },
                                    jit.executed_amount.into(),
                                )
                                .map_err(|err| super::Error(format!("invalid JIT trade: {err}")))?,
                            )),
                        })
                        .try_collect()?,
                    solution
                        .prices
                        .into_iter()
                        .map(|(address, price)| (address.into(), price))
                        .collect(),
                    solution
                        .interactions
                        .into_iter()
                        .map(|interaction| match interaction {
                            Interaction::Custom(interaction) => {
                                Ok(competition::solution::Interaction::Custom(
                                    competition::solution::interaction::Custom {
                                        target: interaction.target.into(),
                                        value: interaction.value.into(),
                                        call_data: interaction.call_data.into(),
                                        allowances: interaction
                                            .allowances
                                            .into_iter()
                                            .map(|allowance| {
                                                eth::Allowance {
                                                    token: allowance.token.into(),
                                                    spender: allowance.spender.into(),
                                                    amount: allowance.amount,
                                                }
                                                .into()
                                            })
                                            .collect(),
                                        inputs: interaction
                                            .inputs
                                            .into_iter()
                                            .map(|input| eth::Asset {
                                                amount: input.amount.into(),
                                                token: input.token.into(),
                                            })
                                            .collect(),
                                        outputs: interaction
                                            .outputs
                                            .into_iter()
                                            .map(|input| eth::Asset {
                                                amount: input.amount.into(),
                                                token: input.token.into(),
                                            })
                                            .collect(),
                                        internalize: interaction.internalize,
                                    },
                                ))
                            }
                            Interaction::Liquidity(interaction) => {
                                let liquidity = liquidity
                                    .iter()
                                    .find(|liquidity| liquidity.id == interaction.id)
                                    .ok_or(super::Error(
                                        "invalid liquidity ID specified in interaction".to_owned(),
                                    ))?
                                    .to_owned();
                                Ok(competition::solution::Interaction::Liquidity(
                                    competition::solution::interaction::Liquidity {
                                        liquidity,
                                        input: eth::Asset {
                                            amount: interaction.input_amount.into(),
                                            token: interaction.input_token.into(),
                                        },
                                        output: eth::Asset {
                                            amount: interaction.output_amount.into(),
                                            token: interaction.output_token.into(),
                                        },
                                        internalize: interaction.internalize,
                                    },
                                ))
                            }
                        })
                        .try_collect()?,
                    solver.clone(),
                    match solution.score {
                        Score::Solver { score } => {
                            competition::solution::SolverScore::Solver(score)
                        }
                        Score::RiskAdjusted {
                            success_probability,
                        } => competition::solution::SolverScore::RiskAdjusted(success_probability),
                    },
                    weth,
                )
                .map_err(|err| match err {
                    competition::solution::SolutionError::InvalidClearingPrices => {
                        super::Error("invalid clearing prices".to_owned())
                    }
                    competition::solution::SolutionError::ProtocolFee(err) => {
                        super::Error(format!("could not incorporate protocol fee: {err}"))
                    }
                    competition::solution::SolutionError::Scoring(err) => {
                        super::Error(format!("could not calculate score: {err}"))
                    }
                })
            })
            .collect()
    }
}

#[serde_as]
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Solutions {
    solutions: Vec<Solution>,
}

#[serde_as]
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Solution {
    id: u64,
    #[serde_as(as = "HashMap<_, serialize::U256>")]
    prices: HashMap<eth::H160, eth::U256>,
    trades: Vec<Trade>,
    interactions: Vec<Interaction>,
    score: Score,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase", deny_unknown_fields)]
enum Trade {
    Fulfillment(Fulfillment),
    Jit(JitTrade),
}

#[serde_as]
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Fulfillment {
    #[serde_as(as = "serialize::Hex")]
    order: [u8; order::UID_LEN],
    #[serde_as(as = "serialize::U256")]
    executed_amount: eth::U256,
    #[serde_as(as = "Option<serialize::U256>")]
    fee: Option<eth::U256>,
}

#[serde_as]
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct JitTrade {
    order: JitOrder,
    #[serde_as(as = "serialize::U256")]
    executed_amount: eth::U256,
}

#[serde_as]
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct JitOrder {
    sell_token: eth::H160,
    buy_token: eth::H160,
    receiver: eth::H160,
    #[serde_as(as = "serialize::U256")]
    sell_amount: eth::U256,
    #[serde_as(as = "serialize::U256")]
    buy_amount: eth::U256,
    valid_to: u32,
    #[serde_as(as = "serialize::Hex")]
    app_data: [u8; order::APP_DATA_LEN],
    #[serde_as(as = "serialize::U256")]
    fee_amount: eth::U256,
    kind: Kind,
    partially_fillable: bool,
    sell_token_balance: SellTokenBalance,
    buy_token_balance: BuyTokenBalance,
    signing_scheme: SigningScheme,
    #[serde_as(as = "serialize::Hex")]
    signature: Vec<u8>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
enum Kind {
    Sell,
    Buy,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase", deny_unknown_fields)]
enum Interaction {
    Liquidity(LiquidityInteraction),
    Custom(CustomInteraction),
}

#[serde_as]
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct LiquidityInteraction {
    internalize: bool,
    #[serde_as(as = "serde_with::DisplayFromStr")]
    id: usize,
    input_token: eth::H160,
    output_token: eth::H160,
    #[serde_as(as = "serialize::U256")]
    input_amount: eth::U256,
    #[serde_as(as = "serialize::U256")]
    output_amount: eth::U256,
}

#[serde_as]
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct CustomInteraction {
    internalize: bool,
    target: eth::H160,
    #[serde_as(as = "serialize::U256")]
    value: eth::U256,
    #[serde_as(as = "serialize::Hex")]
    call_data: Vec<u8>,
    allowances: Vec<Allowance>,
    inputs: Vec<Asset>,
    outputs: Vec<Asset>,
}

#[serde_as]
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Asset {
    token: eth::H160,
    #[serde_as(as = "serialize::U256")]
    amount: eth::U256,
}

#[serde_as]
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Allowance {
    token: eth::H160,
    spender: eth::H160,
    #[serde_as(as = "serialize::U256")]
    amount: eth::U256,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
enum SellTokenBalance {
    #[default]
    Erc20,
    Internal,
    External,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
enum BuyTokenBalance {
    #[default]
    Erc20,
    Internal,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
enum SigningScheme {
    Eip712,
    EthSign,
    PreSign,
    Eip1271,
}

#[serde_as]
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields, tag = "kind")]
pub enum Score {
    Solver {
        #[serde_as(as = "serialize::U256")]
        score: eth::U256,
    },
    #[serde(rename_all = "camelCase")]
    RiskAdjusted { success_probability: f64 },
}
