use {
    crate::{
        domain::{competition, competition::order, eth},
        Ethereum,
    },
    anyhow::Result,
    async_trait::async_trait,
    itertools::Itertools,
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
        signature::EcdsaSignature,
        DomainSeparator,
    },
    number_conversions::u256_to_big_rational,
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
        interactions::allowances::{AllowanceManaging, Allowances, Approval, ApprovalRequest},
        liquidity::{order_converter::OrderConverter, slippage::SlippageCalculator},
        settlement::external_prices::ExternalPrices,
        settlement_simulation::settle_method_builder,
        solver::http_solver::settlement::{convert_settlement, SettlementContext},
    },
    std::{collections::HashSet, sync::Arc},
};

#[derive(Debug, Clone)]
pub struct Settlement {
    settlement: solver::settlement::Settlement,
    contract: contracts::GPv2Settlement,
    solver: eth::Address,
}

impl Settlement {
    pub async fn encode(
        eth: &Ethereum,
        solution: &competition::Solution,
        // TODO I think it's possible to remove this parameter, do this in a follow-up
        // TODO Can I get rid of this and still have the tests pass? If so, go ahead and do it.
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
        let domain = order::signature::domain_separator(
            eth.chain_id(),
            settlement_contract.clone().address().into(),
        );
        let limit_orders = auction
            .orders
            .iter()
            .filter_map(|order| {
                let boundary_order = to_boundary_order(order);
                order_converter.normalize_limit_order(boundary_order).ok()
            })
            .collect_vec();
        let settlement = convert_settlement(
            to_boundary_solution(solution, eth).await?,
            &SettlementContext {
                orders: limit_orders,
                // TODO: #899
                liquidity: Default::default(),
            },
            Arc::new(AllowanceManager),
            Arc::new(order_converter),
            SlippageCalculator {
                relative: to_big_decimal(solution.solver.slippage().relative.clone()),
                absolute: solution.solver.slippage().absolute.map(Into::into),
            }
            .context(&ExternalPrices::try_from_auction_prices(
                native_token.address(),
                auction
                    .tokens
                    .iter()
                    .filter_map(|token| {
                        token
                            .price
                            .map(|price| (token.address.into(), price.into()))
                    })
                    .collect(),
            )?),
            &DomainSeparator(domain.0),
        )
        .await?;
        Ok(Self {
            settlement,
            contract: settlement_contract.to_owned(),
            solver: solution.solver.address(),
        })
    }

    pub fn tx(self) -> eth::Tx {
        let encoded_settlement = self
            .settlement
            .encode(InternalizationStrategy::SkipInternalizableInteraction);
        let builder = settle_method_builder(
            &self.contract,
            encoded_settlement,
            ethcontract::Account::Local(self.solver.into(), None),
        );
        let tx = builder.into_inner();
        eth::Tx {
            from: self.solver,
            to: tx.to.unwrap().into(),
            value: tx.value.unwrap_or_default().into(),
            input: tx.data.unwrap().0,
            access_list: Default::default(),
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
                .tokens
                .iter()
                .filter_map(|token| {
                    token
                        .price
                        .map(|price| (token.address.into(), price.into()))
                })
                .collect(),
        )?;
        let gas_price = u256_to_big_rational(&auction.gas_price.into());
        let inputs = solver::objective_value::Inputs::from_settlement(
            &self.settlement,
            &prices,
            &gas_price,
            &gas.into(),
        );
        Ok(inputs.objective_value().into())
    }
}

fn to_boundary_order(order: &competition::Order) -> Order {
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
                competition::order::Side::Buy => OrderKind::Buy,
                competition::order::Side::Sell => OrderKind::Sell,
            },
            partially_fillable: order.is_partial(),
            sell_token_balance: match order.sell_token_balance {
                competition::order::SellTokenBalance::Erc20 => SellTokenSource::Erc20,
                competition::order::SellTokenBalance::Internal => SellTokenSource::Internal,
                competition::order::SellTokenBalance::External => SellTokenSource::External,
            },
            buy_token_balance: match order.buy_token_balance {
                competition::order::BuyTokenBalance::Erc20 => BuyTokenDestination::Erc20,
                competition::order::BuyTokenBalance::Internal => BuyTokenDestination::Internal,
            },
        },
        metadata: OrderMetadata {
            full_fee_amount: order.fee.solver.into(),
            class: match order.kind {
                competition::order::Kind::Market => OrderClass::Market,
                competition::order::Kind::Liquidity => OrderClass::Liquidity,
                competition::order::Kind::Limit { surplus_fee } => {
                    OrderClass::Limit(LimitOrderClass {
                        surplus_fee: Some(surplus_fee.into()),
                        ..Default::default()
                    })
                }
            },
            creation_date: Default::default(),
            owner: order.signature.signer.into(),
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
            is_liquidity_order: order.is_liquidity(),
        },
        // TODO Different signing schemes imply different sizes of signature data, which indicates
        // that I'm missing an invariant in my types and I need to fix that
        // PreSign, for example, carries no data. Everything should be reflected in the types!
        signature: match order.signature.scheme {
            order::signature::Scheme::Eip712 => model::signature::Signature::Eip712(
                EcdsaSignature::from_bytes(order.signature.data.as_slice().try_into().unwrap()),
            ),
            order::signature::Scheme::EthSign => model::signature::Signature::EthSign(
                EcdsaSignature::from_bytes(order.signature.data.as_slice().try_into().unwrap()),
            ),
            order::signature::Scheme::Eip1271 => {
                model::signature::Signature::Eip1271(order.signature.data.clone())
            }
            order::signature::Scheme::PreSign => model::signature::Signature::PreSign,
        },
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

async fn to_boundary_solution(
    solution: &competition::Solution,
    eth: &Ethereum,
) -> Result<SettledBatchAuctionModel> {
    Ok(SettledBatchAuctionModel {
        orders: solution
            .trades
            .iter()
            .enumerate()
            .filter_map(|(index, trade)| match trade {
                competition::solution::Trade::Fulfillment(fulfillment) => Some((
                    index,
                    ExecutedOrderModel {
                        exec_sell_amount: match fulfillment.order.side {
                            order::Side::Sell => fulfillment.executed.into(),
                            order::Side::Buy => Default::default(),
                        },
                        exec_buy_amount: match fulfillment.order.side {
                            order::Side::Buy => fulfillment.executed.into(),
                            order::Side::Sell => Default::default(),
                        },
                        cost: None,
                        fee: Some(to_token_amount(
                            &fulfillment.order.fee.solver.to_asset(&fulfillment.order),
                        )),
                        exec_plan: None,
                    },
                )),
                competition::solution::Trade::Jit(_) => None,
            })
            .collect(),
        foreign_liquidity_orders: solution
            .trades
            .iter()
            .filter_map(|trade| match trade {
                competition::solution::Trade::Jit(jit) => Some(ExecutedLiquidityOrderModel {
                    order: NativeLiquidityOrder {
                        from: jit.order.signature.signer.into(),
                        data: OrderData {
                            sell_token: jit.order.sell.token.into(),
                            buy_token: jit.order.buy.token.into(),
                            receiver: Some(jit.order.receiver.into()),
                            sell_amount: jit.order.sell.amount,
                            buy_amount: jit.order.buy.amount,
                            valid_to: jit.order.valid_to.into(),
                            app_data: AppId(jit.order.app_data.into()),
                            fee_amount: jit.order.fee.into(),
                            kind: match jit.order.side {
                                competition::order::Side::Buy => OrderKind::Buy,
                                competition::order::Side::Sell => OrderKind::Sell,
                            },
                            partially_fillable: jit.order.partially_fillable,
                            sell_token_balance: match jit.order.sell_token_balance {
                                competition::order::SellTokenBalance::Erc20 => {
                                    SellTokenSource::Erc20
                                }
                                competition::order::SellTokenBalance::Internal => {
                                    SellTokenSource::Internal
                                }
                                competition::order::SellTokenBalance::External => {
                                    SellTokenSource::External
                                }
                            },
                            buy_token_balance: match jit.order.buy_token_balance {
                                competition::order::BuyTokenBalance::Erc20 => {
                                    BuyTokenDestination::Erc20
                                }
                                competition::order::BuyTokenBalance::Internal => {
                                    BuyTokenDestination::Internal
                                }
                            },
                        },
                        signature: Default::default(),
                    },
                    exec_sell_amount: match jit.order.side {
                        order::Side::Sell => jit.executed.into(),
                        order::Side::Buy => Default::default(),
                    },
                    exec_buy_amount: match jit.order.side {
                        order::Side::Buy => jit.executed.into(),
                        order::Side::Sell => Default::default(),
                    },
                }),
                competition::solution::Trade::Fulfillment(_) => None,
            })
            .collect(),
        amms: solution
            .interactions
            .iter()
            .enumerate()
            .filter_map(|(index, interaction)| match interaction {
                competition::solution::Interaction::Liquidity(interaction) => Some((
                    interaction.liquidity.address.into(),
                    UpdatedAmmModel {
                        execution: vec![ExecutedAmmModel {
                            sell_token: interaction.output.token.into(),
                            buy_token: interaction.input.token.into(),
                            exec_sell_amount: interaction.output.amount,
                            exec_buy_amount: interaction.input.amount,
                            exec_plan: ExecutionPlan {
                                coordinates: ExecutionPlanCoordinatesModel {
                                    sequence: 0,
                                    position: index.try_into().unwrap(),
                                },
                                internal: interaction.internalize,
                            },
                        }],
                        cost: None,
                    },
                )),
                competition::solution::Interaction::Custom(_) => None,
            })
            .collect(),
        ref_token: None,
        prices: solution
            .prices
            .iter()
            .map(|(&token, &amount)| (token.into(), amount))
            .collect(),
        approvals: solution
            .approvals(eth)
            .await?
            .into_iter()
            .map(|approval| ApprovalModel {
                token: approval.0.spender.token.into(),
                spender: approval.0.spender.address.into(),
                amount: approval.0.amount,
            })
            .collect(),
        interaction_data: solution
            .interactions
            .iter()
            .enumerate()
            .filter_map(|(index, interaction)| match interaction {
                competition::solution::Interaction::Custom(interaction) => Some(InteractionData {
                    target: interaction.target.into(),
                    value: interaction.value.into(),
                    call_data: interaction.call_data.clone(),
                    inputs: interaction.inputs.iter().map(to_token_amount).collect(),
                    outputs: interaction.outputs.iter().map(to_token_amount).collect(),
                    exec_plan: Some(ExecutionPlan {
                        coordinates: ExecutionPlanCoordinatesModel {
                            sequence: 0,
                            position: index.try_into().unwrap(),
                        },
                        internal: interaction.internalize,
                    }),
                    cost: None,
                }),
                competition::solution::Interaction::Liquidity(_) => None,
            })
            .collect(),
        metadata: None,
        submitter: Default::default(),
    })
}

fn to_token_amount(asset: &eth::Asset) -> TokenAmount {
    TokenAmount {
        amount: asset.amount,
        token: asset.token.into(),
    }
}

fn to_big_decimal(value: bigdecimal::BigDecimal) -> num::BigRational {
    let (x, exp) = value.into_bigint_and_exponent();
    let numerator_bytes = x.to_bytes_le();
    let base = num::bigint::BigInt::from_bytes_le(numerator_bytes.0, &numerator_bytes.1);
    let ten = num::BigRational::new(10.into(), 1.into());
    let numerator = num::BigRational::new(base, 1.into());
    numerator / ten.pow(exp.try_into().expect("should not overflow"))
}

struct AllowanceManager;

#[async_trait]
impl AllowanceManaging for AllowanceManager {
    async fn get_allowances(
        &self,
        _tokens: HashSet<eth::H160>,
        _spender: eth::H160,
    ) -> Result<Allowances> {
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
