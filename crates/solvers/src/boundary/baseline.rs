//! Boundary wrappers around the [`shared`] Baseline solving logic.

use {
    crate::{
        boundary,
        domain::{eth, liquidity, order, solver},
    },
    ethereum_types::{H160, U256},
    model::TokenPair,
    shared::baseline_solver::{self, BaseTokens, BaselineSolvable},
    std::collections::{HashMap, HashSet},
};

pub struct Solver<'a> {
    base_tokens: BaseTokens,
    onchain_liquidity: HashMap<TokenPair, Vec<OnchainLiquidity>>,
    liquidity: HashMap<liquidity::Id, &'a liquidity::Liquidity>,
}

impl<'a> Solver<'a> {
    pub fn new(
        weth: &eth::WethAddress,
        base_tokens: &HashSet<eth::TokenAddress>,
        liquidity: &'a [liquidity::Liquidity],
    ) -> Self {
        Self {
            base_tokens: to_boundary_base_tokens(weth, base_tokens),
            onchain_liquidity: to_boundary_liquidity(liquidity),
            liquidity: liquidity
                .iter()
                .map(|liquidity| (liquidity.id.clone(), liquidity))
                .collect(),
        }
    }

    pub fn route(&self, request: solver::Request, max_hops: usize) -> Option<solver::Route<'a>> {
        let candidates = self.base_tokens.path_candidates_with_hops(
            request.sell.token.0,
            request.buy.token.0,
            max_hops,
        );

        let (segments, _) = match request.side {
            order::Side::Buy => candidates
                .iter()
                .filter_map(|path| {
                    let sell = baseline_solver::estimate_sell_amount(
                        request.buy.amount,
                        path,
                        &self.onchain_liquidity,
                    )?;
                    let segments =
                        self.traverse_path(&sell.path, request.sell.token.0, sell.value)?;

                    let buy = segments.last().map(|segment| segment.output.amount);
                    if buy.map(|buy| buy >= request.buy.amount) != Some(true) {
                        tracing::warn!(
                            ?request,
                            ?segments,
                            "invalid buy estimate does not cover order"
                        );
                        return None;
                    }

                    (sell.value <= request.sell.amount).then_some((segments, sell))
                })
                .min_by_key(|(_, sell)| sell.value)?,
            order::Side::Sell => candidates
                .iter()
                .filter_map(|path| {
                    let buy = baseline_solver::estimate_buy_amount(
                        request.sell.amount,
                        path,
                        &self.onchain_liquidity,
                    )?;
                    let segments =
                        self.traverse_path(&buy.path, request.sell.token.0, request.sell.amount)?;

                    let sell = segments.first().map(|segment| segment.input.amount);
                    if sell.map(|sell| sell >= request.sell.amount) != Some(true) {
                        tracing::warn!(
                            ?request,
                            ?segments,
                            "invalid sell estimate does not cover order"
                        );
                        return None;
                    }

                    (buy.value >= request.buy.amount).then_some((segments, buy))
                })
                .max_by_key(|(_, buy)| buy.value)?,
        };

        solver::Route::new(segments)
    }

    fn traverse_path(
        &self,
        path: &[&OnchainLiquidity],
        mut sell_token: H160,
        mut sell_amount: U256,
    ) -> Option<Vec<solver::Segment<'a>>> {
        let mut segments = Vec::new();
        for liquidity in path {
            let reference_liquidity = self
                .liquidity
                .get(&liquidity.id)
                .expect("boundary liquidity does not match ID");

            let buy_token = liquidity
                .token_pair
                .other(&sell_token)
                .expect("Inconsistent path");
            let buy_amount = liquidity.get_amount_out(buy_token, (sell_amount, sell_token))?;

            segments.push(solver::Segment {
                liquidity: reference_liquidity,
                input: eth::Asset {
                    token: eth::TokenAddress(sell_token),
                    amount: sell_amount,
                },
                output: eth::Asset {
                    token: eth::TokenAddress(buy_token),
                    amount: buy_amount,
                },
                gas: eth::Gas(liquidity.gas_cost().into()),
            });

            sell_token = buy_token;
            sell_amount = buy_amount;
        }
        Some(segments)
    }
}

fn to_boundary_liquidity(
    liquidity: &[liquidity::Liquidity],
) -> HashMap<TokenPair, Vec<OnchainLiquidity>> {
    liquidity
        .iter()
        .fold(HashMap::new(), |mut onchain_liquidity, liquidity| {
            match &liquidity.state {
                liquidity::State::ConstantProduct(pool) => {
                    if let Some(boundary_pool) =
                        boundary::liquidity::constant_product::to_boundary_pool(
                            liquidity.address,
                            pool,
                        )
                    {
                        onchain_liquidity
                            .entry(boundary_pool.tokens)
                            .or_default()
                            .push(OnchainLiquidity {
                                id: liquidity.id.clone(),
                                token_pair: boundary_pool.tokens,
                                source: LiquiditySource::ConstantProduct(boundary_pool),
                            });
                    }
                }
                liquidity::State::WeightedProduct(pool) => {
                    if let Some(boundary_pool) =
                        boundary::liquidity::weighted_product::to_boundary_pool(
                            liquidity.address,
                            pool,
                        )
                    {
                        for pair in pool.reserves.token_pairs() {
                            let token_pair = to_boundary_token_pair(&pair);
                            onchain_liquidity.entry(token_pair).or_default().push(
                                OnchainLiquidity {
                                    id: liquidity.id.clone(),
                                    token_pair,
                                    source: LiquiditySource::WeightedProduct(boundary_pool.clone()),
                                },
                            );
                        }
                    }
                }
                liquidity::State::Stable(pool) => {
                    if let Some(boundary_pool) =
                        boundary::liquidity::stable::to_boundary_pool(liquidity.address, pool)
                    {
                        for pair in pool.reserves.token_pairs() {
                            let token_pair = to_boundary_token_pair(&pair);
                            onchain_liquidity.entry(token_pair).or_default().push(
                                OnchainLiquidity {
                                    id: liquidity.id.clone(),
                                    token_pair,
                                    source: LiquiditySource::Stable(boundary_pool.clone()),
                                },
                            );
                        }
                    }
                }
                liquidity::State::LimitOrder(limit_order) => {
                    if let Some(token_pair) =
                        TokenPair::new(limit_order.maker.token.0, limit_order.taker.token.0)
                    {
                        onchain_liquidity
                            .entry(token_pair)
                            .or_default()
                            .push(OnchainLiquidity {
                                id: liquidity.id.clone(),
                                token_pair,
                                source: LiquiditySource::LimitOrder(limit_order.clone()),
                            })
                    }
                }
                // The baseline solver does not currently support other AMMs.
                _ => {}
            };
            onchain_liquidity
        })
}

#[derive(Debug)]
struct OnchainLiquidity {
    id: liquidity::Id,
    token_pair: TokenPair,
    source: LiquiditySource,
}

#[derive(Debug)]
enum LiquiditySource {
    ConstantProduct(boundary::liquidity::constant_product::Pool),
    WeightedProduct(boundary::liquidity::weighted_product::Pool),
    Stable(boundary::liquidity::stable::Pool),
    LimitOrder(liquidity::limit_order::LimitOrder),
}

impl BaselineSolvable for OnchainLiquidity {
    fn get_amount_out(&self, out_token: H160, input: (U256, H160)) -> Option<U256> {
        match &self.source {
            LiquiditySource::ConstantProduct(pool) => pool.get_amount_out(out_token, input),
            LiquiditySource::WeightedProduct(pool) => pool.get_amount_out(out_token, input),
            LiquiditySource::Stable(pool) => pool.get_amount_out(out_token, input),
            LiquiditySource::LimitOrder(limit_order) => {
                limit_order.get_amount_out(out_token, input)
            }
        }
    }

    fn get_amount_in(&self, in_token: H160, out: (U256, H160)) -> Option<U256> {
        match &self.source {
            LiquiditySource::ConstantProduct(pool) => pool.get_amount_in(in_token, out),
            LiquiditySource::WeightedProduct(pool) => pool.get_amount_in(in_token, out),
            LiquiditySource::Stable(pool) => pool.get_amount_in(in_token, out),
            LiquiditySource::LimitOrder(limit_order) => limit_order.get_amount_in(in_token, out),
        }
    }

    fn gas_cost(&self) -> usize {
        match &self.source {
            LiquiditySource::ConstantProduct(pool) => pool.gas_cost(),
            LiquiditySource::WeightedProduct(pool) => pool.gas_cost(),
            LiquiditySource::Stable(pool) => pool.gas_cost(),
            LiquiditySource::LimitOrder(limit_order) => limit_order.gas_cost(),
        }
    }
}

fn to_boundary_base_tokens(
    weth: &eth::WethAddress,
    base_tokens: &HashSet<eth::TokenAddress>,
) -> BaseTokens {
    let base_tokens = base_tokens.iter().map(|token| token.0).collect::<Vec<_>>();
    BaseTokens::new(weth.0, &base_tokens)
}

fn to_boundary_token_pair(pair: &liquidity::TokenPair) -> TokenPair {
    let (a, b) = pair.get();
    TokenPair::new(a.0, b.0).unwrap()
}
