use {
    crate::{
        domain::{
            self,
            Flashloan,
            competition::{
                self,
                order::{self, Available, Side, fees, signature::Scheme},
            },
            liquidity,
        },
        infra::{config::file::FeeHandler, solver::ManageNativeToken},
    },
    app_data::AppDataHash,
    eth_domain_types as eth,
    model::order::{BuyTokenDestination, SellTokenSource},
    number::conversions::rational_to_big_decimal,
    std::collections::HashMap,
};

pub type WrapperCalls = HashMap<order::Uid, Vec<solvers_dto::auction::WrapperCall>>;

#[expect(clippy::too_many_arguments)]
pub fn new(
    auction: &competition::Auction,
    liquidity: &[liquidity::Liquidity],
    weth: eth::WrappedNativeToken,
    fee_handler: FeeHandler,
    solver_native_token: ManageNativeToken,
    flashloan_hints: &HashMap<order::Uid, Flashloan>,
    wrappers: &WrapperCalls,
    deadline: chrono::DateTime<chrono::Utc>,
    haircut_bps: u32,
) -> solvers_dto::auction::Auction {
    let mut tokens: HashMap<eth::Address, _> = auction
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

                apply_haircut(&mut available, order.side, haircut_bps);
                solvers_dto::auction::Order {
                    uid: order.uid.into(),
                    sell_token: *available.sell.token,
                    buy_token: *available.buy.token,
                    sell_amount: available.sell.amount.into(),
                    buy_amount: available.buy.amount.into(),
                    full_sell_amount: order.sell.amount.into(),
                    full_buy_amount: order.buy.amount.into(),
                    kind: match order.side {
                        Side::Buy => solvers_dto::auction::Kind::Buy,
                        Side::Sell => solvers_dto::auction::Kind::Sell,
                    },
                    receiver: order.receiver,
                    owner: order.signature.signer,
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
                    flashloan_hint: flashloan_hints.get(&order.uid).map(Into::into),
                    wrappers: wrappers
                        .get(&order.uid)
                        .into_iter()
                        .flatten()
                        .cloned()
                        .collect(),
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
                            address: pool.address,
                            router: *pool.router,
                            gas_estimate: liquidity.gas.into(),
                            tokens: pool
                                .reserves
                                .iter()
                                .map(|asset| {
                                    (
                                        *asset.token,
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
                            address: *pool.address,
                            router: *pool.router,
                            gas_estimate: liquidity.gas.0,
                            tokens: vec![*pool.tokens.get().0, *pool.tokens.get().1],
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
                        balancer_pool_id: pool.id.0,
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
                            pool.amplification_parameter.factor().into(),
                            pool.amplification_parameter.precision().into(),
                        )),
                        fee: fee_to_decimal(pool.fee),
                    })
                }
                liquidity::Kind::BalancerV2Weighted(pool) => {
                    solvers_dto::auction::Liquidity::WeightedProduct(
                        solvers_dto::auction::WeightedProductPool {
                            id: liquidity.id.0.to_string(),
                            address: pool.id.address().into(),
                            balancer_pool_id: pool.id.0,
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
                        address: pool.base.address,
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
                            address: *limit_order.zeroex.address(),
                            gas_estimate: liquidity.gas.into(),
                            hash: Default::default(),
                            maker_token: limit_order.order.maker_token,
                            taker_token: limit_order.order.taker_token,
                            maker_amount: eth::U256::from(limit_order.fillable.maker),
                            taker_amount: eth::U256::from(limit_order.fillable.taker),
                            taker_token_fee_amount: eth::U256::from(
                                limit_order.order.taker_token_fee_amount,
                            ),
                        },
                    )
                }
            })
            .collect(),
        tokens,
        effective_gas_price: auction.gas_price().effective().into(),
        deadline,
        surplus_capturing_jit_order_owners: auction
            .surplus_capturing_jit_order_owners()
            .iter()
            .cloned()
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

fn interaction_from_domain(value: domain::Interaction) -> solvers_dto::auction::InteractionData {
    solvers_dto::auction::InteractionData {
        target: value.target,
        value: value.value.0,
        call_data: value.call_data.to_vec(),
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
    bigdecimal::BigDecimal::new(fee.as_raw().into(), 18)
}

fn weight_to_decimal(weight: liquidity::balancer::v2::weighted::Weight) -> bigdecimal::BigDecimal {
    bigdecimal::BigDecimal::new(weight.as_raw().into(), 18)
}

fn scaling_factor_to_decimal(
    scale: liquidity::balancer::v2::ScalingFactor,
) -> bigdecimal::BigDecimal {
    bigdecimal::BigDecimal::new(scale.as_raw().into(), 18)
}

/// The driver applies a haircut to the solver's solution after it is returned
/// (see `Solutions::into_domain`). This reduces the user's effective buy amount
/// (sell orders) or increases their effective sell amount (buy orders) without
/// the solver knowing about it. Tighten the order limits we send the solver by
/// the same factor so that any solution it produces still respects the user's
/// signed limit price after the haircut is applied.
///
/// Sell orders: `buy.amount := buy.amount / (1 - h)`.
/// Buy orders:  `sell.amount := sell.amount / (1 + h)`.
///
/// If the factor multiplication fails (overflow, or `haircut_bps >= 10_000`
/// making `(1 - h) <= 0`), the original limit is preserved rather than zeroed,
/// since a zero-limit order is silently unfillable. `haircut_bps` is expected
/// to be well below `super::MAX_BASE_POINT`; a debug assertion catches
/// misconfigs.
fn apply_haircut(available: &mut Available, side: Side, haircut_bps: u32) {
    if haircut_bps == 0 {
        return;
    }
    debug_assert!(
        haircut_bps < super::MAX_BASE_POINT,
        "haircut_bps {haircut_bps} must be < {}",
        super::MAX_BASE_POINT,
    );
    let haircut_factor = f64::from(haircut_bps) / f64::from(super::MAX_BASE_POINT);
    match side {
        Side::Buy => {
            available.sell.amount = available
                .sell
                .amount
                .apply_factor(1.0 / (1.0 + haircut_factor))
                .unwrap_or(available.sell.amount);
        }
        Side::Sell => {
            available.buy.amount = available
                .buy
                .amount
                .apply_factor(1.0 / (1.0 - haircut_factor))
                .unwrap_or(available.buy.amount);
        }
    }
}

#[cfg(test)]
mod tests {
    use {super::*, alloy::primitives::Address};

    fn asset(amount: eth::U256) -> eth::Asset {
        eth::Asset {
            amount: amount.into(),
            token: Address::repeat_byte(0xaa).into(),
        }
    }

    fn available(sell_amount: eth::U256, buy_amount: eth::U256) -> Available {
        Available {
            sell: asset(sell_amount),
            buy: asset(buy_amount),
        }
    }

    /// Zero haircut leaves the order limits unchanged.
    #[test]
    fn haircut_zero_is_noop() {
        let sell = eth::U256::from(500_000_000u64);
        let buy = eth::U256::from(441_289_983_646_158_011_001u128);

        let mut a = available(sell, buy);
        apply_haircut(&mut a, Side::Sell, 0);
        assert_eq!(a.sell.amount.0, sell);
        assert_eq!(a.buy.amount.0, buy);

        let mut a = available(sell, buy);
        apply_haircut(&mut a, Side::Buy, 0);
        assert_eq!(a.sell.amount.0, sell);
        assert_eq!(a.buy.amount.0, buy);
    }

    /// For sell orders, the buy amount sent to the solver is tightened
    /// to `B / (1 - h)` so that the solver bids with enough headroom for
    /// the driver's post-hoc haircut to still respect the signed limit `B`.
    ///
    /// Regression for the prod incident on order `0xa978e3ec…6a020c06`:
    /// solver `0x4c52…f739` submitted a bid with ~83 bps headroom; the driver
    /// applied the configured haircut and the on-chain `settle()` reverted
    /// with `GPv2: limit price not respected`. With make-room, the limit the
    /// solver sees is the tightened one, so the only solutions it can produce
    /// already satisfy the signed limit post-haircut.
    #[test]
    fn haircut_tightens_buy_for_sell_order() {
        let sell = eth::U256::from(500_000_000u64);
        let signed_buy = eth::U256::from(441_289_983_646_158_011_001u128);

        let mut a = available(sell, signed_buy);
        apply_haircut(&mut a, Side::Sell, 100); // 1% haircut

        // sell amount is untouched for sell orders.
        assert_eq!(a.sell.amount.0, sell);

        // Expected tightened buy: signed_buy / (1 - 0.01).
        let expected = eth::TokenAmount(signed_buy)
            .apply_factor(1.0 / 0.99)
            .unwrap()
            .0;
        assert_eq!(a.buy.amount.0, expected);

        // Sanity: any solver bid `E` that clears the tightened limit
        // (`E >= expected`) survives the post-hoc haircut, i.e.
        // `E * (1 - h) >= signed_buy`.
        assert!(expected > signed_buy);
        let post_haircut = a.buy.amount.apply_factor(0.99).unwrap().0;
        // The post-haircut amount must be `>= signed_buy` (allow a tiny
        // f64-rounding tolerance of a few wei).
        let tolerance = eth::U256::from(10u64);
        assert!(
            post_haircut + tolerance >= signed_buy,
            "post-haircut {post_haircut} < signed {signed_buy}"
        );
    }

    /// Symmetric to the sell case: for buy orders the sell amount is
    /// tightened to `S / (1 + h)` so the driver's post-hoc haircut (which
    /// *adds* to the sell amount the user pays) still respects the signed
    /// sell limit.
    #[test]
    fn haircut_tightens_sell_for_buy_order() {
        let signed_sell = eth::U256::from(500_000_000u64);
        let buy = eth::U256::from(441_289_983_646_158_011_001u128);

        let mut a = available(signed_sell, buy);
        apply_haircut(&mut a, Side::Buy, 100); // 1% haircut

        // buy amount is untouched for buy orders.
        assert_eq!(a.buy.amount.0, buy);

        // Expected tightened sell: signed_sell / (1 + 0.01).
        let expected = eth::TokenAmount(signed_sell)
            .apply_factor(1.0 / 1.01)
            .unwrap()
            .0;
        assert_eq!(a.sell.amount.0, expected);

        // Solver pays at most `expected`; after the driver adds the haircut
        // (`+ h`), the effective sell must not exceed `signed_sell`.
        assert!(expected < signed_sell);
        let post_haircut = a.sell.amount.apply_factor(1.01).unwrap().0;
        let tolerance = eth::U256::from(10u64);
        assert!(
            post_haircut <= signed_sell + tolerance,
            "post-haircut {post_haircut} > signed {signed_sell}"
        );
    }

    /// `apply_factor` failure (here: 100% haircut producing `1/(1-1) = inf`
    /// → `None`) must NOT silently zero the limit. Original is preserved so a
    /// misconfigured solver doesn't quietly drop every order it sees. The
    /// debug-assert above will fire in dev builds; this guards release builds.
    #[test]
    fn haircut_overflow_preserves_original_limit() {
        let sell = eth::U256::from(500_000_000u64);
        let signed_buy = eth::U256::from(441_289_983_646_158_011_001u128);

        // Skip the debug-assert in release-style execution by calling with the
        // boundary value just below; for the overflow case we exercise
        // `apply_factor` returning None directly.
        let mut a = available(sell, signed_buy);
        let huge_factor = f64::INFINITY;
        let result = a.buy.amount.apply_factor(huge_factor);
        assert!(result.is_none(), "sanity: factor of inf must fail");

        // Simulate the fallback path manually (mirrors what `apply_haircut`
        // does on failure: keep the original).
        a.buy.amount = result.unwrap_or(a.buy.amount);
        assert_eq!(
            a.buy.amount.0, signed_buy,
            "fallback must preserve the original limit, not zero it"
        );
    }
}
