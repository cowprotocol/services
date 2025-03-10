use {
    crate::{
        domain::{competition, eth, liquidity},
        infra::{Solver, solver::Config},
        util::Bytes,
    },
    app_data::AppDataHash,
    itertools::Itertools,
    model::{
        DomainSeparator,
        order::{BuyTokenDestination, OrderData, OrderKind, SellTokenSource},
    },
    std::str::FromStr,
};

#[derive(derive_more::From)]
pub struct Solutions(solvers_dto::solution::Solutions);

impl Solutions {
    pub fn into_domain(
        self,
        auction: &competition::Auction,
        liquidity: &[liquidity::Liquidity],
        weth: eth::WethAddress,
        solver: Solver,
        solver_config: &Config,
    ) -> Result<Vec<competition::Solution>, super::Error> {
        self.0.solutions
            .into_iter()
            .map(|solution| {
                competition::Solution::new(
                    competition::solution::Id::new(solution.id),
                    solution
                        .trades
                        .into_iter()
                        .map(|trade| match trade {
                            solvers_dto::solution::Trade::Fulfillment(fulfillment) => {
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
                            solvers_dto::solution::Trade::Jit(jit) => {
                                let jit_order: JitOrder = jit.order.into();
                                Ok(competition::solution::Trade::Jit(
                                competition::solution::trade::Jit::new(
                                    competition::order::Jit {
                                        uid: jit_order.uid(
                                            solver.eth.contracts().settlement_domain_separator(),
                                        )?,
                                        sell: eth::Asset {
                                            amount: jit_order.0.sell_amount.into(),
                                            token: jit_order.0.sell_token.into(),
                                        },
                                        buy: eth::Asset {
                                            amount: jit_order.0.buy_amount.into(),
                                            token: jit_order.0.buy_token.into(),
                                        },
                                        receiver: jit_order.0.receiver.into(),
                                        partially_fillable: jit_order.0.partially_fillable,
                                        valid_to: jit_order.0.valid_to.into(),
                                        app_data: jit_order.0.app_data.into(),
                                        side: match jit_order.0.kind {
                                            solvers_dto::solution::Kind::Sell => competition::order::Side::Sell,
                                            solvers_dto::solution::Kind::Buy => competition::order::Side::Buy,
                                        },
                                        sell_token_balance: match jit_order.0.sell_token_balance {
                                            solvers_dto::solution::SellTokenBalance::Erc20 => {
                                                competition::order::SellTokenBalance::Erc20
                                            }
                                            solvers_dto::solution::SellTokenBalance::Internal => {
                                                competition::order::SellTokenBalance::Internal
                                            }
                                            solvers_dto::solution::SellTokenBalance::External => {
                                                competition::order::SellTokenBalance::External
                                            }
                                        },
                                        buy_token_balance: match jit_order.0.buy_token_balance {
                                            solvers_dto::solution::BuyTokenBalance::Erc20 => {
                                                competition::order::BuyTokenBalance::Erc20
                                            }
                                            solvers_dto::solution::BuyTokenBalance::Internal => {
                                                competition::order::BuyTokenBalance::Internal
                                            }
                                        },
                                        signature: jit_order.signature(
                                            solver.eth.contracts().settlement_domain_separator(),
                                        )?,
                                    },
                                    jit.executed_amount.into(),
                                    jit.fee.unwrap_or_default().into(),
                                )
                                .map_err(|err| super::Error(format!("invalid JIT trade: {err}")))?,
                            ))},
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
                            call_data: Bytes(interaction.calldata),
                        })
                        .collect(),
                    solution
                        .interactions
                        .into_iter()
                        .map(|interaction| match interaction {
                            solvers_dto::solution::Interaction::Custom(interaction) => {
                                Ok(competition::solution::Interaction::Custom(
                                    competition::solution::interaction::Custom {
                                        target: interaction.target.into(),
                                        value: interaction.value.into(),
                                        call_data: interaction.calldata.into(),
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
                            solvers_dto::solution::Interaction::Liquidity(interaction) => {
                                let liquidity_id = usize::from_str(&interaction.id).map_err(|_| super::Error("invalid liquidity ID format".to_owned()))?;
                                let liquidity = liquidity
                                    .iter()
                                    .find(|liquidity| liquidity.id == liquidity_id)
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
                            call_data: Bytes(interaction.calldata),
                        })
                        .collect(),
                    solver.clone(),
                    weth,
                    solution.gas.map(|gas| eth::Gas(gas.into())),
                    solver_config.fee_handler,
                    auction.surplus_capturing_jit_order_owners(),
                    solution
                        .flashloans
                        .into_iter()
                        .map(|flashloan| eth::Flashloan {
                            lender: flashloan.lender.into(),
                            borrower: flashloan.borrower.into(),
                            token: flashloan.token.into(),
                            amount: flashloan.amount.into(),
                        })
                        .collect(),
                )
                .map_err(|err| match err {
                    competition::solution::error::Solution::InvalidClearingPrices => {
                        super::Error("invalid clearing prices".to_owned())
                    }
                    competition::solution::error::Solution::ProtocolFee(err) => {
                        super::Error(format!("could not incorporate protocol fee: {err}"))
                    }
                    competition::solution::error::Solution::InvalidJitTrade(err) => {
                        super::Error(format!("invalid jit trade: {err}"))
                    }
                })
            })
            .collect()
    }
}

#[derive(derive_more::From)]
pub struct JitOrder(solvers_dto::solution::JitOrder);

impl JitOrder {
    fn raw_order_data(&self) -> OrderData {
        OrderData {
            sell_token: self.0.sell_token,
            buy_token: self.0.buy_token,
            receiver: Some(self.0.receiver),
            sell_amount: self.0.sell_amount,
            buy_amount: self.0.buy_amount,
            valid_to: self.0.valid_to,
            app_data: AppDataHash(self.0.app_data),
            fee_amount: 0.into(),
            kind: match self.0.kind {
                solvers_dto::solution::Kind::Sell => OrderKind::Sell,
                solvers_dto::solution::Kind::Buy => OrderKind::Buy,
            },
            partially_fillable: self.0.partially_fillable,
            sell_token_balance: match self.0.sell_token_balance {
                solvers_dto::solution::SellTokenBalance::Erc20 => SellTokenSource::Erc20,
                solvers_dto::solution::SellTokenBalance::Internal => SellTokenSource::Internal,
                solvers_dto::solution::SellTokenBalance::External => SellTokenSource::External,
            },
            buy_token_balance: match self.0.buy_token_balance {
                solvers_dto::solution::BuyTokenBalance::Erc20 => BuyTokenDestination::Erc20,
                solvers_dto::solution::BuyTokenBalance::Internal => BuyTokenDestination::Internal,
            },
        }
    }

    fn signature(
        &self,
        domain_separator: &eth::DomainSeparator,
    ) -> Result<competition::order::Signature, super::Error> {
        let mut signature = competition::order::Signature {
            scheme: match self.0.signing_scheme {
                solvers_dto::solution::SigningScheme::Eip712 => {
                    competition::order::signature::Scheme::Eip712
                }
                solvers_dto::solution::SigningScheme::EthSign => {
                    competition::order::signature::Scheme::EthSign
                }
                solvers_dto::solution::SigningScheme::PreSign => {
                    competition::order::signature::Scheme::PreSign
                }
                solvers_dto::solution::SigningScheme::Eip1271 => {
                    competition::order::signature::Scheme::Eip1271
                }
            },
            data: self.0.signature.clone().into(),
            signer: Default::default(),
        };

        let signer = signature
            .to_boundary_signature()
            .recover_owner(
                self.0.signature.as_slice(),
                &DomainSeparator(domain_separator.0),
                &self.raw_order_data().hash_struct(),
            )
            .map_err(|e| super::Error(e.to_string()))?;

        if matches!(
            self.0.signing_scheme,
            solvers_dto::solution::SigningScheme::Eip1271
        ) {
            // For EIP-1271 signatures the encoding logic prepends the signer to the raw
            // signature bytes. This leads to the owner being encoded twice in
            // the final settlement calldata unless we remove that from the raw
            // data.
            signature.data = Bytes(self.0.signature[20..].to_vec());
        }

        signature.signer = signer.into();

        Ok(signature)
    }

    fn uid(&self, domain: &eth::DomainSeparator) -> Result<competition::order::Uid, super::Error> {
        let order_data = self.raw_order_data();
        let signature = self.signature(domain)?;
        Ok(order_data
            .uid(&DomainSeparator(domain.0), &signature.signer.into())
            .0
            .into())
    }
}
