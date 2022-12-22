use {
    crate::{
        logic::{competition, eth},
        Ethereum,
        Solver,
    },
    anyhow::Result,
    async_trait::async_trait,
    model::{
        app_id::AppId,
        order::{
            BuyTokenDestination,
            Interactions,
            LimitOrderClass,
            Order,
            OrderClass,
            OrderData,
            OrderKind,
            OrderMetadata,
            OrderUid,
            SellTokenSource,
        },
        DomainSeparator,
    },
    number_conversions::u256_to_big_rational,
    primitive_types::H160,
    shared::http_solver::model::{
        ApprovalModel,
        ExecutedAmmModel,
        ExecutedLiquidityOrderModel,
        ExecutedOrderModel,
        ExecutionPlan,
        ExecutionPlanCoordinatesModel,
        InteractionData,
        InternalizationStrategy,
        NativeLiquidityOrder,
        SettledBatchAuctionModel,
        TokenAmount,
        UpdatedAmmModel,
    },
    solver::{
        driver::solver_settlements::RatedSettlement,
        interactions::allowances::{AllowanceManaging, Allowances, Approval, ApprovalRequest},
        liquidity::{order_converter::OrderConverter, slippage::SlippageCalculator},
        settlement::external_prices::ExternalPrices,
        settlement_simulation::settle_method_builder,
        solver::http_solver::settlement::{convert_settlement, SettlementContext},
    },
    std::{collections::HashSet, sync::Arc},
};

#[derive(Debug)]
pub struct Settlement {
    settlement: solver::settlement::Settlement,
    contract: contracts::GPv2Settlement,
    solver_account: eth::Account,
}

impl Settlement {
    pub async fn encode(
        eth: &Ethereum,
        solver: &Solver,
        solution: competition::Solution,
        auction: &competition::Auction,
    ) -> Result<Self> {
        let native_token = eth.contracts().weth();
        let order_converter = OrderConverter {
            native_token: native_token.clone(),
            // Fee is already scaled by the autopilot, so this can be set to exactly 1.
            fee_objective_scaling_factor: 1.,
            min_order_age: Default::default(),
        };
        let settlement_contract = eth.contracts().settlement();
        let domain = eth.domain_separator(settlement_contract.clone().address().into());
        let limit_orders = auction
            .orders
            .iter()
            .filter_map(|order| {
                let boundary_order = into_boundary_order(order);
                order_converter.normalize_limit_order(boundary_order).ok()
            })
            .collect();
        let settlement = convert_settlement(
            into_boundary_solution(solution),
            SettlementContext {
                orders: limit_orders,
                // TODO: #899
                liquidity: Default::default(),
            },
            Arc::new(AllowanceManager),
            Arc::new(order_converter),
            SlippageCalculator {
                relative: solver.slippage().relative.clone(),
                absolute: solver.slippage().absolute.map(Into::into),
            }
            .context(&ExternalPrices::try_from_auction_prices(
                native_token.address(),
                auction
                    .prices
                    .iter()
                    .map(|(&token, &amount)| (token.into(), amount.into()))
                    .collect(),
            )?),
            &DomainSeparator(domain.0),
        )
        .await?;
        Ok(Self {
            settlement,
            contract: settlement_contract,
            solver_account: solver.account(),
        })
    }

    pub async fn tx(self) -> eth::Tx {
        let encoded_settlement = self
            .settlement
            .encode(InternalizationStrategy::SkipInternalizableInteraction);
        let builder = settle_method_builder(
            &self.contract,
            encoded_settlement,
            match self.solver_account {
                eth::Account::PrivateKey(private_key) => ethcontract::Account::Offline(
                    ethcontract::PrivateKey::from_raw(private_key.into())
                        .expect("private key was already validated"),
                    None,
                ),
                eth::Account::Address(address) => ethcontract::Account::Local(address.into(), None),
            },
        );
        let tx = builder.into_inner();
        eth::Tx {
            from: tx.from.unwrap().address().into(),
            to: tx.to.unwrap().into(),
            value: tx.value.unwrap().into(),
            input: tx.data.unwrap().0,
        }
    }

    pub async fn score(
        self,
        eth: &Ethereum,
        auction: &competition::Auction,
        gas: eth::Gas,
    ) -> Result<competition::solution::Score> {
        let prices = ExternalPrices::try_from_auction_prices(
            eth.contracts().weth().address(),
            auction
                .prices
                .iter()
                .map(|(&token, &amount)| (token.into(), amount.into()))
                .collect(),
        )?;
        let surplus = self.settlement.total_surplus(&prices);
        let scaled_solver_fees = self.settlement.total_scaled_unsubsidized_fees(&prices);
        let unscaled_subsidized_fee = self.settlement.total_unscaled_subsidized_fees(&prices);
        Ok(RatedSettlement {
            id: 0,
            settlement: self.settlement,
            surplus,
            unscaled_subsidized_fee,
            scaled_unsubsidized_fee: scaled_solver_fees,
            gas_estimate: gas.into(),
            gas_price: u256_to_big_rational(&auction.gas_price.into()),
        }
        .objective_value()
        .into())
    }
}

fn into_boundary_order(order: &competition::auction::Order) -> Order {
    Order {
        data: OrderData {
            sell_token: order.sell.token.into(),
            buy_token: order.buy.token.into(),
            sell_amount: order.sell.amount,
            buy_amount: order.buy.amount,
            fee_amount: order.fee.user.into(),
            receiver: order.receiver.map(Into::into),
            valid_to: order.valid_to.into(),
            app_data: AppId(order.app_data.into()),
            kind: match order.side {
                competition::Side::Buy => OrderKind::Buy,
                competition::Side::Sell => OrderKind::Sell,
            },
            partially_fillable: order.partial,

            sell_token_balance: match order.sell_source {
                competition::auction::order::SellSource::Erc20 => SellTokenSource::Erc20,
                competition::auction::order::SellSource::Internal => SellTokenSource::Internal,
                competition::auction::order::SellSource::External => SellTokenSource::External,
            },
            buy_token_balance: match order.buy_destination {
                competition::auction::order::BuyDestination::Erc20 => BuyTokenDestination::Erc20,
                competition::auction::order::BuyDestination::Internal => {
                    BuyTokenDestination::Internal
                }
            },
        },
        metadata: OrderMetadata {
            full_fee_amount: order.fee.solver.into(),
            class: match order.kind {
                competition::auction::order::Kind::Market => OrderClass::Market,
                competition::auction::order::Kind::Liquidity => OrderClass::Liquidity,
                competition::auction::order::Kind::Limit { surplus_fee } => {
                    OrderClass::Limit(LimitOrderClass {
                        surplus_fee: Some(surplus_fee.into()),
                        ..Default::default()
                    })
                }
            },
            creation_date: Default::default(),
            owner: order.from.into(),
            uid: OrderUid(order.uid.into()),
            available_balance: Default::default(),
            executed_buy_amount: Default::default(),
            executed_sell_amount: Default::default(),
            executed_sell_amount_before_fees: Default::default(),
            executed_fee_amount: Default::default(),
            invalidated: Default::default(),
            status: Default::default(),
            settlement_contract: Default::default(),
            ethflow_data: Default::default(),
            onchain_user: Default::default(),
            onchain_order_data: Default::default(),
            is_liquidity_order: order.kind.is_liquidity(),
        },
        signature: Default::default(),
        interactions: Interactions {
            pre: order
                .interactions
                .iter()
                .map(|interaction| model::interaction::InteractionData {
                    target: interaction.target.into(),
                    value: interaction.value.into(),
                    call_data: interaction.call_data.clone(),
                })
                .collect(),
        },
    }
}

fn into_boundary_solution(solution: competition::Solution) -> SettledBatchAuctionModel {
    SettledBatchAuctionModel {
        orders: solution
            .orders
            .into_iter()
            .map(|(index, order)| {
                (
                    index,
                    ExecutedOrderModel {
                        exec_sell_amount: order.sell,
                        exec_buy_amount: order.buy,
                        cost: None,
                        fee: order.fee.map(|fee| TokenAmount {
                            amount: fee.amount,
                            token: fee.token.into(),
                        }),
                        exec_plan: order.plan.map(|plan| ExecutionPlan {
                            coordinates: ExecutionPlanCoordinatesModel {
                                sequence: plan.sequence,
                                position: plan.position,
                            },
                            internal: plan.internal,
                        }),
                    },
                )
            })
            .collect(),
        foreign_liquidity_orders: solution
            .jit_orders
            .into_iter()
            .map(|jit| ExecutedLiquidityOrderModel {
                order: NativeLiquidityOrder {
                    from: jit.from.into(),
                    data: OrderData {
                        sell_token: jit.sell.token.into(),
                        buy_token: jit.buy.token.into(),
                        receiver: jit.receiver.map(Into::into),
                        sell_amount: jit.sell.amount,
                        buy_amount: jit.buy.amount,
                        valid_to: jit.valid_to.into(),
                        app_data: AppId(jit.app_data.into()),
                        fee_amount: jit.fee.into(),
                        kind: match jit.side {
                            competition::Side::Buy => OrderKind::Buy,
                            competition::Side::Sell => OrderKind::Sell,
                        },
                        partially_fillable: jit.partially_fillable,
                        sell_token_balance: match jit.sell_source {
                            competition::solution::SellSource::Erc20 => SellTokenSource::Erc20,
                            competition::solution::SellSource::Internal => {
                                SellTokenSource::Internal
                            }
                            competition::solution::SellSource::External => {
                                SellTokenSource::External
                            }
                        },
                        buy_token_balance: match jit.buy_destination {
                            competition::solution::BuyDestination::Erc20 => {
                                BuyTokenDestination::Erc20
                            }
                            competition::solution::BuyDestination::Internal => {
                                BuyTokenDestination::Internal
                            }
                        },
                    },
                    signature: Default::default(),
                },
                exec_sell_amount: jit.executed_sell_amount,
                exec_buy_amount: jit.executed_buy_amount,
            })
            .collect(),
        amms: solution
            .amms
            .into_iter()
            .map(|(address, amms)| {
                (
                    address.into(),
                    UpdatedAmmModel {
                        execution: amms
                            .into_iter()
                            .map(|amm| ExecutedAmmModel {
                                sell_token: amm.sell.token.into(),
                                buy_token: amm.buy.token.into(),
                                exec_sell_amount: amm.sell.amount,
                                exec_buy_amount: amm.buy.amount,
                                exec_plan: ExecutionPlan {
                                    coordinates: ExecutionPlanCoordinatesModel {
                                        sequence: amm.sequence,
                                        position: amm.position,
                                    },
                                    internal: amm.internal,
                                },
                            })
                            .collect(),
                        // TODO @nlordell is this right...?
                        // We're getting rid of this and it's only needed for price estimation, so
                        // shouldn't be used here?
                        cost: None,
                    },
                )
            })
            .collect(),
        // TODO I believe we're getting rid of this as well
        ref_token: None,
        prices: solution
            .prices
            .into_iter()
            .map(|(token, amount)| (token.into(), amount.into()))
            .collect(),
        approvals: solution
            .approvals
            .into_iter()
            .map(|approval| ApprovalModel {
                token: approval.0.spender.token.into(),
                spender: approval.0.spender.address.into(),
                amount: approval.0.amount,
            })
            .collect(),
        interaction_data: solution
            .interactions
            .into_iter()
            .map(|interaction| InteractionData {
                target: interaction.inner.target.into(),
                value: interaction.inner.value.into(),
                call_data: interaction.inner.call_data,
                inputs: interaction
                    .inputs
                    .into_iter()
                    .map(|input| TokenAmount {
                        amount: input.amount,
                        token: input.token.into(),
                    })
                    .collect(),
                outputs: interaction
                    .outputs
                    .into_iter()
                    .map(|output| TokenAmount {
                        amount: output.amount,
                        token: output.token.into(),
                    })
                    .collect(),
                // TODO I have no clue why there's an exec plan here? This is a single interaction,
                // what sort of ordering is needed? I don't know.
                exec_plan: Default::default(),
                cost: None,
            })
            .collect(),
        // TODO I think this is also not needed, but please somebody double-check this
        metadata: None,
        submitter: Default::default(),
    }
}

struct AllowanceManager;

#[async_trait]
impl AllowanceManaging for AllowanceManager {
    async fn get_allowances(&self, _tokens: HashSet<H160>, _spender: H160) -> Result<Allowances> {
        unimplemented!("this is not supposed to be called")
    }

    async fn get_approvals(&self, requests: &[ApprovalRequest]) -> Result<Vec<Approval>> {
        Ok(requests
            .iter()
            .map(|request| Approval {
                token: request.token,
                spender: request.spender,
            })
            .collect())
    }
}
