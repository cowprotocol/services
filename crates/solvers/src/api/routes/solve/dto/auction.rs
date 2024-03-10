use {
    crate::{
        api::routes::Error,
        domain::{auction, eth, liquidity, order},
        util::conv,
    },
    itertools::Itertools,
    solvers_dto::auction::*,
};

/// Converts a data transfer object into its domain object representation.
pub fn to_domain(auction: &Auction) -> Result<auction::Auction, Error> {
    Ok(auction::Auction {
        id: match auction.id {
            Some(id) => auction::Id::Solve(id),
            None => auction::Id::Quote,
        },
        tokens: auction::Tokens(
            auction
                .tokens
                .iter()
                .map(|(address, token)| {
                    (
                        eth::TokenAddress(*address),
                        auction::Token {
                            decimals: token.decimals,
                            symbol: token.symbol.clone(),
                            reference_price: token
                                .reference_price
                                .map(eth::Ether)
                                .map(auction::Price),
                            available_balance: token.available_balance,
                            trusted: token.trusted,
                        },
                    )
                })
                .collect(),
        ),
        orders: auction
            .orders
            .iter()
            .map(|order| order::Order {
                uid: order::Uid(order.uid),
                sell: eth::Asset {
                    token: eth::TokenAddress(order.sell_token),
                    amount: order.sell_amount,
                },
                buy: eth::Asset {
                    token: eth::TokenAddress(order.buy_token),
                    amount: order.buy_amount,
                },
                side: match order.kind {
                    Kind::Buy => order::Side::Buy,
                    Kind::Sell => order::Side::Sell,
                },
                class: match order.class {
                    Class::Market => order::Class::Market,
                    Class::Limit => order::Class::Limit,
                    Class::Liquidity => order::Class::Liquidity,
                },
                fee: order::Fee(order.fee_amount),
                partially_fillable: order.partially_fillable,
            })
            .collect(),
        liquidity: auction
            .liquidity
            .iter()
            .map(|liquidity| match liquidity {
                Liquidity::ConstantProduct(liquidity) => {
                    constant_product_pool::to_domain(liquidity)
                }
                Liquidity::WeightedProduct(liquidity) => {
                    weighted_product_pool::to_domain(liquidity)
                }
                Liquidity::Stable(liquidity) => stable_pool::to_domain(liquidity),
                Liquidity::ConcentratedLiquidity(liquidity) => {
                    concentrated_liquidity_pool::to_domain(liquidity)
                }
                Liquidity::LimitOrder(liquidity) => Ok(foreign_limit_order::to_domain(liquidity)),
            })
            .try_collect()?,
        gas_price: auction::GasPrice(eth::Ether(auction.effective_gas_price)),
        deadline: auction::Deadline(auction.deadline),
    })
}

mod constant_product_pool {
    use {super::*, itertools::Itertools};

    pub fn to_domain(pool: &ConstantProductPool) -> Result<liquidity::Liquidity, Error> {
        let reserves = {
            let (a, b) = pool
                .tokens
                .iter()
                .map(|(token, reserve)| eth::Asset {
                    token: eth::TokenAddress(*token),
                    amount: reserve.balance,
                })
                .collect_tuple()
                .ok_or("invalid number of constant product tokens")?;
            liquidity::constant_product::Reserves::new(a, b)
                .ok_or("invalid constant product pool reserves")?
        };

        Ok(liquidity::Liquidity {
            id: liquidity::Id(pool.id.clone()),
            address: pool.address,
            gas: eth::Gas(pool.gas_estimate),
            state: liquidity::State::ConstantProduct(liquidity::constant_product::Pool {
                reserves,
                fee: conv::decimal_to_rational(&pool.fee).ok_or("invalid constant product fee")?,
            }),
        })
    }
}

mod weighted_product_pool {
    use super::*;
    pub fn to_domain(pool: &WeightedProductPool) -> Result<liquidity::Liquidity, Error> {
        let reserves = {
            let entries = pool
                .tokens
                .iter()
                .map(|(address, token)| {
                    Ok(liquidity::weighted_product::Reserve {
                        asset: eth::Asset {
                            token: eth::TokenAddress(*address),
                            amount: token.balance,
                        },
                        weight: conv::decimal_to_rational(&token.weight)
                            .ok_or("invalid token weight")?,
                        scale: conv::decimal_to_rational(&token.scaling_factor)
                            .and_then(liquidity::ScalingFactor::new)
                            .ok_or("invalid token scaling factor")?,
                    })
                })
                .collect::<Result<Vec<_>, Error>>()?;
            liquidity::weighted_product::Reserves::new(entries)
                .ok_or("duplicate weighted token addresses")?
        };

        Ok(liquidity::Liquidity {
            id: liquidity::Id(pool.id.clone()),
            address: pool.address,
            gas: eth::Gas(pool.gas_estimate),
            state: liquidity::State::WeightedProduct(liquidity::weighted_product::Pool {
                reserves,
                fee: conv::decimal_to_rational(&pool.fee).ok_or("invalid weighted product fee")?,
                version: match pool.version {
                    WeightedProductVersion::V0 => liquidity::weighted_product::Version::V0,
                    WeightedProductVersion::V3Plus => liquidity::weighted_product::Version::V3Plus,
                },
            }),
        })
    }
}

mod stable_pool {
    use super::*;

    pub fn to_domain(pool: &StablePool) -> Result<liquidity::Liquidity, Error> {
        let reserves = {
            let entries = pool
                .tokens
                .iter()
                .map(|(address, token)| {
                    Ok(liquidity::stable::Reserve {
                        asset: eth::Asset {
                            token: eth::TokenAddress(*address),
                            amount: token.balance,
                        },
                        scale: conv::decimal_to_rational(&token.scaling_factor)
                            .and_then(liquidity::ScalingFactor::new)
                            .ok_or("invalid token scaling factor")?,
                    })
                })
                .collect::<Result<Vec<_>, Error>>()?;
            liquidity::stable::Reserves::new(entries).ok_or("duplicate stable token addresses")?
        };

        Ok(liquidity::Liquidity {
            id: liquidity::Id(pool.id.clone()),
            address: pool.address,
            gas: eth::Gas(pool.gas_estimate),
            state: liquidity::State::Stable(liquidity::stable::Pool {
                reserves,
                amplification_parameter: conv::decimal_to_rational(&pool.amplification_parameter)
                    .ok_or("invalid amplification parameter")?,
                fee: conv::decimal_to_rational(&pool.fee).ok_or("invalid stable pool fee")?,
            }),
        })
    }
}

mod concentrated_liquidity_pool {
    use {super::*, itertools::Itertools};

    pub fn to_domain(pool: &ConcentratedLiquidityPool) -> Result<liquidity::Liquidity, Error> {
        let tokens = {
            let (a, b) = pool
                .tokens
                .iter()
                .copied()
                .map(eth::TokenAddress)
                .collect_tuple()
                .ok_or("invalid number of concentrated liquidity pool tokens")?;
            liquidity::TokenPair::new(a, b)
                .ok_or("duplicate concentrated liquidity pool token address")?
        };

        Ok(liquidity::Liquidity {
            id: liquidity::Id(pool.id.clone()),
            address: pool.address,
            gas: eth::Gas(pool.gas_estimate),
            state: liquidity::State::Concentrated(liquidity::concentrated::Pool {
                tokens,
                sqrt_price: liquidity::concentrated::SqrtPrice(pool.sqrt_price),
                liquidity: liquidity::concentrated::Amount(pool.liquidity),
                tick: liquidity::concentrated::Tick(pool.tick),
                liquidity_net: pool
                    .liquidity_net
                    .iter()
                    .map(|(tick, liquidity)| {
                        (
                            liquidity::concentrated::Tick(*tick),
                            liquidity::concentrated::LiquidityNet(*liquidity),
                        )
                    })
                    .collect(),
                fee: liquidity::concentrated::Fee(
                    conv::decimal_to_rational(&pool.fee)
                        .ok_or("invalid concentrated liquidity pool fee")?,
                ),
            }),
        })
    }
}

mod foreign_limit_order {
    use super::*;

    pub fn to_domain(order: &ForeignLimitOrder) -> liquidity::Liquidity {
        liquidity::Liquidity {
            id: liquidity::Id(order.id.clone()),
            address: order.address,
            gas: eth::Gas(order.gas_estimate),
            state: liquidity::State::LimitOrder(liquidity::limit_order::LimitOrder {
                maker: eth::Asset {
                    token: eth::TokenAddress(order.maker_token),
                    amount: order.maker_amount,
                },
                taker: eth::Asset {
                    token: eth::TokenAddress(order.taker_token),
                    amount: order.taker_amount,
                },
                fee: liquidity::limit_order::TakerAmount(order.taker_token_fee_amount),
            }),
        }
    }
}
