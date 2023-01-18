use {
    crate::{
        domain::{
            competition::{self, order},
            eth,
            liquidity,
        },
        infra::blockchain,
        Ethereum,
    },
    anyhow::{Context, Result},
    model::{
        app_id::AppId,
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
    number_conversions::u256_to_big_rational,
    shared::http_solver::model::InternalizationStrategy,
    solver::{
        driver::solver_settlements::RatedSettlement,
        interactions::Erc20ApproveInteraction,
        liquidity::{
            order_converter::OrderConverter,
            slippage::{SlippageCalculator, SlippageContext},
            AmmOrderExecution,
        },
        settlement::external_prices::ExternalPrices,
        settlement_simulation::settle_method_builder,
    },
    web3::Web3,
};

#[derive(Debug)]
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

        let clearing_prices = solution
            .prices
            .iter()
            .map(|(&token, &amount)| (token.into(), amount))
            .collect();
        let mut settlement = solver::settlement::Settlement::new(clearing_prices);

        for trade in &solution.trades {
            let (boundary_order, executed_amount) = match trade {
                competition::solution::Trade::Fulfillment(trade) => {
                    // TODO: The `http_solver` module filters out orders with 0
                    // executed amounts which seems weird to me... why is a
                    // solver specifying trades with 0 executed amounts?
                    anyhow::ensure!(
                        !eth::U256::from(trade.executed).is_zero(),
                        "unexpected empty execution",
                    );

                    (to_boundary_order(&trade.order), trade.executed.into())
                }
                competition::solution::Trade::Jit(trade) => (
                    to_boundary_jit_order(&DomainSeparator(domain.0), &trade.order),
                    trade.executed.into(),
                ),
            };

            let boundary_limit_order = order_converter.normalize_limit_order(boundary_order)?;
            settlement.with_liquidity(&boundary_limit_order, executed_amount)?;
        }

        let approvals = solution.approvals(eth).await?;
        for approval in approvals {
            settlement
                .encoder
                .append_to_execution_plan(Erc20ApproveInteraction {
                    token: eth.contract_at::<contracts::ERC20>(approval.0.spender.token.into()),
                    spender: approval.0.spender.address.into(),
                    amount: approval.0.amount,
                });
        }

        let slippage_calculator = SlippageCalculator {
            relative: to_big_decimal(solution.solver.slippage().relative.clone()),
            absolute: solution.solver.slippage().absolute.map(Into::into),
        };
        let external_prices = ExternalPrices::try_from_auction_prices(
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
        )?;
        let slippage_context = slippage_calculator.context(&external_prices);

        for interaction in &solution.interactions {
            let boundary_interaction = to_boundary_interaction(
                &slippage_context,
                settlement_contract.address().into(),
                interaction,
            )?;
            settlement.encoder.append_to_execution_plan_internalizable(
                boundary_interaction,
                interaction.internalize(),
            );
        }

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
        signature: to_boundary_signature(&order.signature),
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

fn to_boundary_jit_order(domain: &DomainSeparator, order: &order::Jit) -> Order {
    let data = OrderData {
        sell_token: order.sell.token.into(),
        buy_token: order.buy.token.into(),
        receiver: Some(order.receiver.into()),
        sell_amount: order.sell.amount,
        buy_amount: order.buy.amount,
        valid_to: order.valid_to.into(),
        app_data: AppId(order.app_data.into()),
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
        settlement_contract: Default::default(),
        // For other metdata fields, the default value is correct.
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
            EcdsaSignature::from_bytes(signature.data.as_slice().try_into().unwrap()),
        ),
        order::signature::Scheme::EthSign => model::signature::Signature::EthSign(
            EcdsaSignature::from_bytes(signature.data.as_slice().try_into().unwrap()),
        ),
        order::signature::Scheme::Eip1271 => {
            model::signature::Signature::Eip1271(signature.data.clone())
        }
        order::signature::Scheme::PreSign => model::signature::Signature::PreSign,
    }
}

fn to_boundary_interaction(
    slippage_context: &SlippageContext,
    settlement_contract: eth::ContractAddress,
    interaction: &competition::solution::Interaction,
) -> Result<InteractionData> {
    match interaction {
        competition::solution::Interaction::Custom(custom) => Ok(InteractionData {
            target: custom.target.into(),
            value: custom.value.into(),
            call_data: custom.call_data.clone(),
        }),
        competition::solution::Interaction::Liquidity(liquidity) => {
            let boundary_execution =
                slippage_context.apply_to_amm_execution(AmmOrderExecution {
                    input_max: (liquidity.input.token.into(), liquidity.input.amount),
                    output: (liquidity.output.token.into(), liquidity.output.amount),
                    internalizable: interaction.internalize(),
                })?;

            let input = liquidity::MaxInput(eth::Asset {
                token: boundary_execution.input_max.0.into(),
                amount: boundary_execution.input_max.1,
            });
            let output = liquidity::ExactOutput(eth::Asset {
                token: boundary_execution.output.0.into(),
                amount: boundary_execution.output.1,
            });

            let interaction = match &liquidity.liquidity.data {
                liquidity::Data::UniswapV2(pool) => pool
                    .swap(&input, &output, &settlement_contract.into())
                    .context("invalid uniswap V2 execution")?,
                liquidity::Data::UniswapV3(_) => todo!(),
                liquidity::Data::BalancerV2Stable(_) => todo!(),
                liquidity::Data::BalancerV2Weighted(_) => todo!(),
                liquidity::Data::Swapr(_) => todo!(),
                liquidity::Data::ZeroEx(_) => todo!(),
            };

            Ok(InteractionData {
                target: interaction.target.into(),
                value: interaction.value.into(),
                call_data: interaction.call_data,
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

impl blockchain::contracts::ContractAt for contracts::ERC20 {
    fn at(web3: &Web3<web3::transports::Http>, address: eth::ContractAddress) -> Self {
        contracts::ERC20::at(web3, address.into())
    }
}
