use {
    crate::{
        domain::{
            competition::{
                self,
                auction,
                order,
                solution::{
                    self,
                    settlement::{self, Internalization},
                },
            },
            eth,
            liquidity,
        },
        infra::Ethereum,
    },
    anyhow::{anyhow, ensure, Context, Result},
    bigdecimal::Signed,
    model::{
        app_id::AppDataHash,
        interaction::InteractionData,
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
    number_conversions::{big_rational_to_u256, u256_to_big_rational},
    shared::{
        external_prices::ExternalPrices,
        http_solver::model::{InternalizationStrategy, TokenAmount},
    },
    solver::{
        interactions::Erc20ApproveInteraction,
        liquidity::{
            order_converter::OrderConverter,
            slippage::{SlippageCalculator, SlippageContext},
            AmmOrderExecution,
            LimitOrderExecution,
        },
        settlement_simulation::settle_method_builder,
    },
    std::sync::Arc,
};

#[derive(Debug, Clone)]
pub struct Settlement {
    pub(super) inner: solver::settlement::Settlement,
    pub solver: eth::Address,
    risk: solution::Risk,
}

impl Settlement {
    pub async fn encode(
        eth: &Ethereum,
        solution: &competition::Solution,
        auction: &competition::Auction,
    ) -> Result<Self> {
        let native_token = eth.contracts().weth();
        let order_converter = OrderConverter {
            native_token: native_token.clone(),
        };

        let settlement_contract = eth.contracts().settlement();
        let domain = order::signature::domain_separator(
            eth.chain_id(),
            settlement_contract.clone().address().into(),
        );

        let mut settlement = solver::settlement::Settlement::new(
            solution
                .clearing_prices()?
                .into_iter()
                .map(|asset| (asset.token.into(), asset.amount.into()))
                .collect(),
        );

        for trade in solution.trades() {
            let (boundary_order, execution) = match trade {
                competition::solution::Trade::Fulfillment(trade) => {
                    // TODO: The `http_solver` module filters out orders with 0
                    // executed amounts which seems weird to me... why is a
                    // solver specifying trades with 0 executed amounts?
                    if eth::U256::from(trade.executed()).is_zero() {
                        return Err(anyhow!("unexpected empty execution"));
                    }

                    (
                        to_boundary_order(trade.order()),
                        LimitOrderExecution {
                            filled: trade.executed().into(),
                            solver_fee: trade.solver_fee().into(),
                        },
                    )
                }
                competition::solution::Trade::Jit(trade) => (
                    to_boundary_jit_order(&DomainSeparator(domain.0), trade.order()),
                    LimitOrderExecution {
                        filled: trade.executed().into(),
                        solver_fee: 0.into(),
                    },
                ),
            };

            let boundary_limit_order = order_converter.normalize_limit_order(
                solver::order_balance_filter::BalancedOrder::full(boundary_order),
            )?;
            settlement.with_liquidity(&boundary_limit_order, execution)?;
        }

        let approvals = solution.approvals(eth).await?;
        for approval in approvals {
            settlement
                .encoder
                .append_to_execution_plan(Arc::new(Erc20ApproveInteraction {
                    token: eth.contract_at(approval.0.spender.token.into()),
                    spender: approval.0.spender.address.into(),
                    amount: approval.0.amount,
                }));
        }

        let slippage_calculator = SlippageCalculator {
            relative: to_big_decimal(solution.solver().slippage().relative.clone()),
            absolute: solution.solver().slippage().absolute.map(Into::into),
        };
        let external_prices = ExternalPrices::try_from_auction_prices(
            native_token.address(),
            auction
                .tokens()
                .iter()
                .filter_map(|token| {
                    token
                        .price
                        .map(|price| (token.address.into(), price.into()))
                })
                .collect(),
        )?;
        let slippage_context = slippage_calculator.context(&external_prices);

        for interaction in solution.interactions() {
            let boundary_interaction = to_boundary_interaction(
                &slippage_context,
                settlement_contract.address().into(),
                interaction,
            )?;
            settlement.encoder.append_to_execution_plan_internalizable(
                Arc::new(boundary_interaction),
                interaction.internalize(),
            );
        }

        Ok(Self {
            inner: settlement,
            solver: solution.solver().address(),
            risk: solution.risk(),
        })
    }

    pub fn tx(
        &self,
        auction_id: auction::Id,
        contract: &contracts::GPv2Settlement,
        internalization: Internalization,
    ) -> eth::Tx {
        let encoded_settlement = self.inner.clone().encode(match internalization {
            settlement::Internalization::Enable => {
                InternalizationStrategy::SkipInternalizableInteraction
            }
            settlement::Internalization::Disable => InternalizationStrategy::EncodeAllInteractions,
        });
        let builder = settle_method_builder(
            contract,
            encoded_settlement,
            ethcontract::Account::Local(self.solver.into(), None),
        );
        let tx = builder.into_inner();
        let mut input = tx.data.unwrap().0;
        input.extend(auction_id.to_be_bytes());
        eth::Tx {
            from: self.solver,
            to: tx.to.unwrap().into(),
            value: tx.value.unwrap_or_default().into(),
            input: input.into(),
            access_list: Default::default(),
        }
    }

    pub fn score(
        &self,
        eth: &Ethereum,
        auction: &competition::Auction,
        gas: eth::Gas,
    ) -> Result<competition::solution::Score> {
        let prices = ExternalPrices::try_from_auction_prices(
            eth.contracts().weth().address(),
            auction
                .tokens()
                .iter()
                .filter_map(|token| {
                    token
                        .price
                        .map(|price| (token.address.into(), price.into()))
                })
                .collect(),
        )?;
        let gas_price = u256_to_big_rational(&auction.gas_price().effective().into());
        let inputs = solver::objective_value::Inputs::from_settlement(
            &self.inner,
            &prices,
            gas_price,
            &gas.into(),
        );
        ensure!(!inputs.objective_value().is_negative(), "negative score");
        let objective_value = big_rational_to_u256(&inputs.objective_value())?;
        Ok((objective_value - self.risk.0).into())
    }

    pub fn merge(self, other: Self) -> Result<Self> {
        self.inner.merge(other.inner).map(|inner| Self {
            inner,
            solver: self.solver,
            risk: self.risk.merge(other.risk),
        })
    }
}

fn to_boundary_order(order: &competition::Order) -> Order {
    Order {
        data: OrderData {
            sell_token: order.sell.token.into(),
            buy_token: order.buy.token.into(),
            sell_amount: order.sell.amount.into(),
            buy_amount: order.buy.amount.into(),
            fee_amount: order.fee.user.into(),
            receiver: order.receiver.map(Into::into),
            valid_to: order.valid_to.into(),
            app_data: AppDataHash(order.app_data.into()),
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
            full_fee_amount: Default::default(),
            solver_fee: order.fee.solver.into(),
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
            full_app_data: Default::default(),
        },
        signature: to_boundary_signature(&order.signature),
        interactions: Interactions {
            pre: order
                .pre_interactions
                .iter()
                .map(|interaction| model::interaction::InteractionData {
                    target: interaction.target.into(),
                    value: interaction.value.into(),
                    call_data: interaction.call_data.clone().into(),
                })
                .collect(),
            post: order
                .post_interactions
                .iter()
                .map(|interaction| model::interaction::InteractionData {
                    target: interaction.target.into(),
                    value: interaction.value.into(),
                    call_data: interaction.call_data.clone().into(),
                })
                .collect(),
        },
    }
}

fn to_boundary_jit_order(domain: &DomainSeparator, order: &order::Jit) -> Order {
    let data = OrderData {
        sell_token: order.sell.token.into(),
        buy_token: order.buy.token.into(),
        receiver: Some(order.receiver.into()),
        sell_amount: order.sell.amount.into(),
        buy_amount: order.buy.amount.into(),
        valid_to: order.valid_to.into(),
        app_data: AppDataHash(order.app_data.into()),
        fee_amount: order.fee.into(),
        kind: match order.side {
            competition::order::Side::Buy => OrderKind::Buy,
            competition::order::Side::Sell => OrderKind::Sell,
        },
        partially_fillable: order.partially_fillable,
        sell_token_balance: match order.sell_token_balance {
            competition::order::SellTokenBalance::Erc20 => SellTokenSource::Erc20,
            competition::order::SellTokenBalance::Internal => SellTokenSource::Internal,
            competition::order::SellTokenBalance::External => SellTokenSource::External,
        },
        buy_token_balance: match order.buy_token_balance {
            competition::order::BuyTokenBalance::Erc20 => BuyTokenDestination::Erc20,
            competition::order::BuyTokenBalance::Internal => BuyTokenDestination::Internal,
        },
    };
    let metadata = OrderMetadata {
        owner: order.signature.signer.into(),
        full_fee_amount: order.fee.into(),
        // All foreign orders **MUST** be liquidity, this is
        // important so they cannot be used to affect the objective.
        class: OrderClass::Liquidity,
        // Not needed for encoding but nice to have for logs and competition info.
        uid: data.uid(domain, &order.signature.signer.into()),
        // These fields do not seem to be used at all for order
        // encoding, so we just use the default values.
        ..Default::default()
    };
    let signature = to_boundary_signature(&order.signature);

    Order {
        data,
        metadata,
        signature,
        interactions: Interactions::default(),
    }
}

fn to_boundary_signature(signature: &order::Signature) -> model::signature::Signature {
    // TODO Different signing schemes imply different sizes of signature data, which
    // indicates that I'm missing an invariant in my types and I need to fix
    // that PreSign, for example, carries no data. Everything should be
    // reflected in the types!
    match signature.scheme {
        order::signature::Scheme::Eip712 => model::signature::Signature::Eip712(
            EcdsaSignature::from_bytes(signature.data.0.as_slice().try_into().unwrap()),
        ),
        order::signature::Scheme::EthSign => model::signature::Signature::EthSign(
            EcdsaSignature::from_bytes(signature.data.0.as_slice().try_into().unwrap()),
        ),
        order::signature::Scheme::Eip1271 => {
            model::signature::Signature::Eip1271(signature.data.clone().into())
        }
        order::signature::Scheme::PreSign => model::signature::Signature::PreSign,
    }
}

pub fn to_boundary_interaction(
    slippage_context: &SlippageContext,
    settlement_contract: eth::ContractAddress,
    interaction: &competition::solution::Interaction,
) -> Result<InteractionData> {
    match interaction {
        competition::solution::Interaction::Custom(custom) => Ok(InteractionData {
            target: custom.target.into(),
            value: custom.value.into(),
            call_data: custom.call_data.clone().into(),
        }),
        competition::solution::Interaction::Liquidity(liquidity) => {
            let boundary_execution =
                slippage_context.apply_to_amm_execution(AmmOrderExecution {
                    input_max: TokenAmount::new(
                        liquidity.input.token.into(),
                        liquidity.input.amount,
                    ),
                    output: TokenAmount::new(
                        liquidity.output.token.into(),
                        liquidity.output.amount,
                    ),
                    internalizable: interaction.internalize(),
                })?;

            let input = liquidity::MaxInput(eth::Asset {
                token: boundary_execution.input_max.token.into(),
                amount: boundary_execution.input_max.amount.into(),
            });
            let output = liquidity::ExactOutput(eth::Asset {
                token: boundary_execution.output.token.into(),
                amount: boundary_execution.output.amount.into(),
            });

            let interaction = match &liquidity.liquidity.kind {
                liquidity::Kind::UniswapV2(pool) => pool
                    .swap(&input, &output, &settlement_contract.into())
                    .context("invalid uniswap V2 execution")?,
                liquidity::Kind::UniswapV3(pool) => pool
                    .swap(&input, &output, &settlement_contract.into())
                    .context("invalid uniswap v3 execution")?,
                liquidity::Kind::BalancerV2Stable(_) => todo!(),
                liquidity::Kind::BalancerV2Weighted(pool) => pool
                    .swap(&input, &output, &settlement_contract.into())
                    .context("invalid balancer v2 execution")?,
                liquidity::Kind::Swapr(pool) => pool
                    .swap(&input, &output, &settlement_contract.into())
                    .context("invalid swapr execution")?,
                liquidity::Kind::ZeroEx(_) => todo!(),
            };

            Ok(InteractionData {
                target: interaction.target.into(),
                value: interaction.value.into(),
                call_data: interaction.call_data.into(),
            })
        }
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
