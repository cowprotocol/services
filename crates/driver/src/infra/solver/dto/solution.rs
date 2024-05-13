use {
    crate::{
        domain::{competition, competition::order, eth, liquidity},
        infra::{solver::Config, Solver},
        util::{serialize, Bytes},
    },
    app_data::AppDataHash,
    itertools::Itertools,
    model::{
        interaction::InteractionData,
        order::{BuyTokenDestination, OrderData, OrderKind, SellTokenSource},
        DomainSeparator,
    },
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
        solver_config: &Config,
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
                                        signature: {
                                            let mut signature = competition::order::Signature {
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
                                                data: jit.order.signature.clone().into(),
                                                signer: Default::default(),
                                            };

                                            // Recover the signer from the order signature
                                            let signer = Self::recover_signer_from_jit_trade_order(&jit, &signature, solver.eth.contracts().settlement_domain_separator())?;
                                            signature.signer = signer;

                                            signature
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
                        .pre_interactions
                        .into_iter()
                        .map(|interaction| eth::Interaction {
                            target: interaction.target.into(),
                            value: interaction.value.into(),
                            call_data: Bytes(interaction.call_data),
                        })
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
                    solution
                        .post_interactions
                        .into_iter()
                        .map(|interaction| eth::Interaction {
                            target: interaction.target.into(),
                            value: interaction.value.into(),
                            call_data: Bytes(interaction.call_data),
                        })
                        .collect(),
                    solver.clone(),
                    weth,
                    solution.gas.map(|gas| eth::Gas(gas.into())),
                    solver_config.fee_handler,
                )
                .map_err(|err| match err {
                    competition::solution::error::Solution::InvalidClearingPrices => {
                        super::Error("invalid clearing prices".to_owned())
                    }
                    competition::solution::error::Solution::ProtocolFee(err) => {
                        super::Error(format!("could not incorporate protocol fee: {err}"))
                    }
                })
            })
            .collect()
    }

    /// Function to recover the signer of a JIT order
    fn recover_signer_from_jit_trade_order(
        jit: &JitTrade,
        signature: &competition::order::Signature,
        domain: &eth::DomainSeparator,
    ) -> Result<eth::Address, super::Error> {
        let order_data = OrderData {
            sell_token: jit.order.sell_token,
            buy_token: jit.order.buy_token,
            receiver: Some(jit.order.receiver),
            sell_amount: jit.order.sell_amount,
            buy_amount: jit.order.buy_amount,
            valid_to: jit.order.valid_to,
            app_data: AppDataHash(jit.order.app_data),
            fee_amount: jit.order.fee_amount,
            kind: match jit.order.kind {
                Kind::Sell => OrderKind::Sell,
                Kind::Buy => OrderKind::Buy,
            },
            partially_fillable: jit.order.partially_fillable,
            sell_token_balance: match jit.order.sell_token_balance {
                SellTokenBalance::Erc20 => SellTokenSource::Erc20,
                SellTokenBalance::Internal => SellTokenSource::Internal,
                SellTokenBalance::External => SellTokenSource::External,
            },
            buy_token_balance: match jit.order.buy_token_balance {
                BuyTokenBalance::Erc20 => BuyTokenDestination::Erc20,
                BuyTokenBalance::Internal => BuyTokenDestination::Internal,
            },
        };

        signature
            .to_boundary_signature()
            .recover_owner(
                jit.order.signature.as_slice(),
                &DomainSeparator(domain.0),
                &order_data.hash_struct(),
            )
            .map_err(|e| super::Error(e.to_string()))
            .map(Into::into)
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
    #[serde(default)]
    pre_interactions: Vec<InteractionData>,
    interactions: Vec<Interaction>,
    #[serde(default)]
    post_interactions: Vec<InteractionData>,
    // TODO: remove this once all solvers are updated to not return the score
    // https://github.com/cowprotocol/services/issues/2588
    #[allow(dead_code)]
    score: Option<Score>,
    gas: Option<u64>,
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
