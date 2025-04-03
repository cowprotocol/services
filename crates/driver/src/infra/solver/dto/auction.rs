use {
    crate::{
        domain::{
            competition,
            competition::{
                order,
                order::{Side, fees, signature::Scheme},
            },
            eth::{self},
            liquidity,
        },
        infra::{config::file::FeeHandler, solver::ManageNativeToken},
        util::conv::{rational_to_big_decimal, u256::U256Ext},
    },
    app_data::AppDataHash,
    model::order::{BuyTokenDestination, SellTokenSource},
    std::collections::HashMap,
};

pub fn new(
    auction: &competition::Auction,
    liquidity: &[liquidity::Liquidity],
    weth: eth::WethAddress,
    fee_handler: FeeHandler,
    solver_native_token: ManageNativeToken,
    flashloans_enabled: bool,
    flashloan_default_lender: Option<eth::ContractAddress>,
) -> solvers_dto::auction::Auction {
    let mut tokens: HashMap<eth::H160, _> = auction
        .tokens()
        .iter()
        .map(|token| {
            (
                token.address.into(),
                solvers_dto::auction::Token {
                    decimals: token.decimals,
                    symbol: token.symbol.clone(),
                    reference_price: token.price.map(Into::into),
                    available_balance: token.available_balance,
                    trusted: token.trusted,
                },
            )
        })
        .collect();

    // Make sure that we have at least empty entries for all tokens for
    // which we are providing liquidity.
    for token in liquidity
        .iter()
        .flat_map(|liquidity| match &liquidity.kind {
            liquidity::Kind::UniswapV2(pool) => pool.reserves.iter().map(|r| r.token).collect(),
            liquidity::Kind::UniswapV3(pool) => vec![pool.tokens.get().0, pool.tokens.get().1],
            liquidity::Kind::BalancerV2Stable(pool) => pool.reserves.tokens().collect(),
            liquidity::Kind::BalancerV2Weighted(pool) => pool.reserves.tokens().collect(),
            liquidity::Kind::Swapr(pool) => pool.base.reserves.iter().map(|r| r.token).collect(),
            liquidity::Kind::ZeroEx(limit_order) => {
                vec![
                    limit_order.order.maker_token.into(),
                    limit_order.order.taker_token.into(),
                ]
            }
        })
    {
        tokens.entry(token.into()).or_insert_with(Default::default);
    }

    solvers_dto::auction::Auction {
        id: auction.id().as_ref().map(|id| id.0),
        orders: auction
            .orders()
            .iter()
            .map(|order| {
                let mut available = order.available();

                if solver_native_token.wrap_address {
                    available.buy.token = available.buy.token.as_erc20(weth)
                }
                // In case of volume based fees, fee withheld by driver might be higher than the
                // surplus of the solution. This would lead to violating limit prices when
                // driver tries to withhold the volume based fee. To avoid this, we artificially
                // adjust the order limit amounts (make then worse) before sending to solvers,
                // to force solvers to only submit solutions with enough surplus to cover the
                // fee.
                //
                // https://github.com/cowprotocol/services/issues/2440
                if fee_handler == FeeHandler::Driver {
                    order.protocol_fees.iter().for_each(|protocol_fee| {
                        if let fees::FeePolicy::Volume { factor } = protocol_fee {
                            match order.side {
                                Side::Buy => {
                                    // reduce sell amount by factor
                                    available.sell.amount = available
                                        .sell
                                        .amount
                                        .apply_factor(1.0 / (1.0 + factor))
                                        .unwrap_or_default();
                                }
                                Side::Sell => {
                                    // increase buy amount by factor
                                    available.buy.amount = available
                                        .buy
                                        .amount
                                        .apply_factor(1.0 / (1.0 - factor))
                                        .unwrap_or_default();
                                }
                            }
                        }
                    })
                }
                solvers_dto::auction::Order {
                    uid: order.uid.into(),
                    sell_token: available.sell.token.into(),
                    buy_token: available.buy.token.into(),
                    sell_amount: available.sell.amount.into(),
                    buy_amount: available.buy.amount.into(),
                    full_sell_amount: order.sell.amount.into(),
                    full_buy_amount: order.buy.amount.into(),
                    kind: match order.side {
                        Side::Buy => solvers_dto::auction::Kind::Buy,
                        Side::Sell => solvers_dto::auction::Kind::Sell,
                    },
                    receiver: order.receiver.map(Into::into),
                    owner: order.signature.signer.into(),
                    partially_fillable: order.is_partial(),
                    class: match order.kind {
                        order::Kind::Market => solvers_dto::auction::Class::Market,
                        order::Kind::Limit => solvers_dto::auction::Class::Limit,
                    },
                    pre_interactions: order
                        .pre_interactions
                        .iter()
                        .cloned()
                        .map(interaction_from_domain)
                        .collect::<Vec<_>>(),
                    post_interactions: order
                        .post_interactions
                        .iter()
                        .cloned()
                        .map(interaction_from_domain)
                        .collect::<Vec<_>>(),
                    sell_token_source: sell_token_source_from_domain(
                        order.sell_token_balance.into(),
                    ),
                    buy_token_destination: buy_token_destination_from_domain(
                        order.buy_token_balance.into(),
                    ),
                    fee_policies: (fee_handler == FeeHandler::Solver).then_some(
                        order
                            .protocol_fees
                            .iter()
                            .cloned()
                            .map(fee_policy_from_domain)
                            .collect(),
                    ),
                    app_data: AppDataHash(order.app_data.hash().0.into()),
                    flashloan_hint: flashloans_enabled
                        .then(|| {
                            order.app_data.flashloan().and_then(|flashloan| {
                                let lender =
                                    flashloan.lender.or(flashloan_default_lender.map(|l| l.0));
                                lender.map(|lender| solvers_dto::auction::FlashloanHint {
                                    lender,
                                    borrower: flashloan.borrower.unwrap_or(order.uid.owner().0),
                                    token: flashloan.token,
                                    amount: flashloan.amount,
                                })
                            })
                        })
                        .flatten(),
                    signature: order.signature.data.clone().into(),
                    signing_scheme: match order.signature.scheme {
                        Scheme::Eip712 => solvers_dto::auction::SigningScheme::Eip712,
                        Scheme::EthSign => solvers_dto::auction::SigningScheme::EthSign,
                        Scheme::Eip1271 => solvers_dto::auction::SigningScheme::Eip1271,
                        Scheme::PreSign => solvers_dto::auction::SigningScheme::PreSign,
                    },
                    valid_to: order.valid_to.into(),
                }
            })
            .collect(),
        liquidity: liquidity
            .iter()
            .map(|liquidity| match &liquidity.kind {
                liquidity::Kind::UniswapV2(pool) => {
                    solvers_dto::auction::Liquidity::ConstantProduct(
                        solvers_dto::auction::ConstantProductPool {
                            id: liquidity.id.0.to_string(),
                            address: pool.address.into(),
                            router: pool.router.into(),
                            gas_estimate: liquidity.gas.into(),
                            tokens: pool
                                .reserves
                                .iter()
                                .map(|asset| {
                                    (
                                        asset.token.into(),
                                        solvers_dto::auction::ConstantProductReserve {
                                            balance: asset.amount.into(),
                                        },
                                    )
                                })
                                .collect(),
                            fee: bigdecimal::BigDecimal::new(3.into(), 3),
                        },
                    )
                }
                liquidity::Kind::UniswapV3(pool) => {
                    solvers_dto::auction::Liquidity::ConcentratedLiquidity(
                        solvers_dto::auction::ConcentratedLiquidityPool {
                            id: liquidity.id.0.to_string(),
                            address: pool.address.0,
                            router: pool.router.into(),
                            gas_estimate: liquidity.gas.0,
                            tokens: vec![pool.tokens.get().0.into(), pool.tokens.get().1.into()],
                            sqrt_price: pool.sqrt_price.0,
                            liquidity: pool.liquidity.0,
                            tick: pool.tick.0,
                            liquidity_net: pool
                                .liquidity_net
                                .iter()
                                .map(|(key, value)| (key.0, value.0))
                                .collect(),
                            fee: rational_to_big_decimal(&pool.fee.0),
                        },
                    )
                }
                liquidity::Kind::BalancerV2Stable(pool) => {
                    solvers_dto::auction::Liquidity::Stable(solvers_dto::auction::StablePool {
                        id: liquidity.id.0.to_string(),
                        address: pool.id.address().into(),
                        balancer_pool_id: pool.id.into(),
                        gas_estimate: liquidity.gas.into(),
                        tokens: pool
                            .reserves
                            .iter()
                            .map(|r| {
                                (
                                    r.asset.token.into(),
                                    solvers_dto::auction::StableReserve {
                                        balance: r.asset.amount.into(),
                                        scaling_factor: scaling_factor_to_decimal(r.scale),
                                    },
                                )
                            })
                            .collect(),
                        amplification_parameter: rational_to_big_decimal(&num::BigRational::new(
                            pool.amplification_parameter.factor().to_big_int(),
                            pool.amplification_parameter.precision().to_big_int(),
                        )),
                        fee: fee_to_decimal(pool.fee),
                    })
                }
                liquidity::Kind::BalancerV2Weighted(pool) => {
                    solvers_dto::auction::Liquidity::WeightedProduct(
                        solvers_dto::auction::WeightedProductPool {
                            id: liquidity.id.0.to_string(),
                            address: pool.id.address().into(),
                            balancer_pool_id: pool.id.into(),
                            gas_estimate: liquidity.gas.into(),
                            tokens: pool
                                .reserves
                                .iter()
                                .map(|r| {
                                    (
                                        r.asset.token.into(),
                                        solvers_dto::auction::WeightedProductReserve {
                                            balance: r.asset.amount.into(),
                                            scaling_factor: scaling_factor_to_decimal(r.scale),
                                            weight: weight_to_decimal(r.weight),
                                        },
                                    )
                                })
                                .collect(),
                            fee: fee_to_decimal(pool.fee),
                            version: match pool.version {
                                liquidity::balancer::v2::weighted::Version::V0 => {
                                    solvers_dto::auction::WeightedProductVersion::V0
                                }
                                liquidity::balancer::v2::weighted::Version::V3Plus => {
                                    solvers_dto::auction::WeightedProductVersion::V3Plus
                                }
                            },
                        },
                    )
                }
                liquidity::Kind::Swapr(pool) => solvers_dto::auction::Liquidity::ConstantProduct(
                    solvers_dto::auction::ConstantProductPool {
                        id: liquidity.id.0.to_string(),
                        address: pool.base.address.into(),
                        router: pool.base.router.into(),
                        gas_estimate: liquidity.gas.into(),
                        tokens: pool
                            .base
                            .reserves
                            .iter()
                            .map(|asset| {
                                (
                                    asset.token.into(),
                                    solvers_dto::auction::ConstantProductReserve {
                                        balance: asset.amount.into(),
                                    },
                                )
                            })
                            .collect(),
                        fee: bigdecimal::BigDecimal::new(pool.fee.bps().into(), 4),
                    },
                ),
                liquidity::Kind::ZeroEx(limit_order) => {
                    solvers_dto::auction::Liquidity::LimitOrder(
                        solvers_dto::auction::ForeignLimitOrder {
                            id: liquidity.id.0.to_string(),
                            address: limit_order.zeroex.address(),
                            gas_estimate: liquidity.gas.into(),
                            hash: Default::default(),
                            maker_token: limit_order.order.maker_token,
                            taker_token: limit_order.order.taker_token,
                            maker_amount: limit_order.fillable.maker.into(),
                            taker_amount: limit_order.fillable.taker.into(),
                            taker_token_fee_amount: limit_order.order.taker_token_fee_amount.into(),
                        },
                    )
                }
            })
            .collect(),
        tokens,
        effective_gas_price: auction.gas_price().effective().into(),
        deadline: auction.deadline().solvers(),
        surplus_capturing_jit_order_owners: auction
            .surplus_capturing_jit_order_owners()
            .iter()
            .cloned()
            .map(Into::into)
            .collect::<Vec<_>>(),
    }
}

fn fee_policy_from_domain(value: fees::FeePolicy) -> solvers_dto::auction::FeePolicy {
    match value {
        order::FeePolicy::Surplus {
            factor,
            max_volume_factor,
        } => solvers_dto::auction::FeePolicy::Surplus {
            factor,
            max_volume_factor,
        },
        order::FeePolicy::PriceImprovement {
            factor,
            max_volume_factor,
            quote,
        } => solvers_dto::auction::FeePolicy::PriceImprovement {
            factor,
            max_volume_factor,
            quote: solvers_dto::auction::Quote {
                sell_amount: quote.sell.amount.into(),
                buy_amount: quote.buy.amount.into(),
                fee: quote.fee.amount.into(),
            },
        },
        order::FeePolicy::Volume { factor } => solvers_dto::auction::FeePolicy::Volume { factor },
    }
}

fn interaction_from_domain(value: eth::Interaction) -> solvers_dto::auction::InteractionData {
    solvers_dto::auction::InteractionData {
        target: value.target.0,
        value: value.value.0,
        call_data: value.call_data.0,
    }
}

fn sell_token_source_from_domain(value: SellTokenSource) -> solvers_dto::auction::SellTokenSource {
    match value {
        SellTokenSource::Erc20 => solvers_dto::auction::SellTokenSource::Erc20,
        SellTokenSource::External => solvers_dto::auction::SellTokenSource::External,
        SellTokenSource::Internal => solvers_dto::auction::SellTokenSource::Internal,
    }
}

fn buy_token_destination_from_domain(
    value: BuyTokenDestination,
) -> solvers_dto::auction::BuyTokenDestination {
    match value {
        BuyTokenDestination::Erc20 => solvers_dto::auction::BuyTokenDestination::Erc20,
        BuyTokenDestination::Internal => solvers_dto::auction::BuyTokenDestination::Internal,
    }
}

fn fee_to_decimal(fee: liquidity::balancer::v2::Fee) -> bigdecimal::BigDecimal {
    bigdecimal::BigDecimal::new(fee.as_raw().to_big_int(), 18)
}

fn weight_to_decimal(weight: liquidity::balancer::v2::weighted::Weight) -> bigdecimal::BigDecimal {
    bigdecimal::BigDecimal::new(weight.as_raw().to_big_int(), 18)
}

fn scaling_factor_to_decimal(
    scale: liquidity::balancer::v2::ScalingFactor,
) -> bigdecimal::BigDecimal {
    bigdecimal::BigDecimal::new(scale.as_raw().to_big_int(), 18)
}
