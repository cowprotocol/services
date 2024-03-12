//! Boundary wrappers around the [`shared`] Baseline solving logic.

use {
    crate::{
        boundary,
        domain::{eth, liquidity, order, solver::baseline},
    },
    ethereum_types::{H160, U256},
    model::TokenPair,
    shared::baseline_solver::{self, BaseTokens, BaselineSolvable},
    std::collections::{HashMap, HashSet},
};

pub struct Solver<'a> {
    base_tokens: BaseTokens,
    amms: HashMap<TokenPair, Vec<Amm>>,
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
            amms: to_boundary_amms(liquidity),
            liquidity: liquidity
                .iter()
                .map(|liquidity| (liquidity.id.clone(), liquidity))
                .collect(),
        }
    }

    pub fn route(
        &self,
        request: baseline::Request,
        max_hops: usize,
    ) -> Option<baseline::Route<'a>> {
        if request.sell.token == request.buy.token {
            return Some(baseline::Route::new(vec![]));
        }
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
                        &self.amms,
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
                        &self.amms,
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

        Some(baseline::Route::new(segments))
    }

    fn traverse_path(
        &self,
        path: &[&Amm],
        mut sell_token: H160,
        mut sell_amount: U256,
    ) -> Option<Vec<baseline::Segment<'a>>> {
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

            segments.push(baseline::Segment {
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

fn to_boundary_amms(liquidity: &[liquidity::Liquidity]) -> HashMap<TokenPair, Vec<Amm>> {
    liquidity
        .iter()
        .fold(HashMap::new(), |mut amms, liquidity| {
            match &liquidity.state {
                liquidity::State::ConstantProduct(pool) => {
                    if let Some(boundary_pool) =
                        boundary::liquidity::constant_product::to_boundary_pool(
                            liquidity.address,
                            pool,
                        )
                    {
                        amms.entry(boundary_pool.tokens).or_default().push(Amm {
                            id: liquidity.id.clone(),
                            token_pair: boundary_pool.tokens,
                            pool: Pool::ConstantProduct(boundary_pool),
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
                            amms.entry(token_pair).or_default().push(Amm {
                                id: liquidity.id.clone(),
                                token_pair,
                                pool: Pool::WeightedProduct(boundary_pool.clone()),
                            });
                        }
                    }
                }
                liquidity::State::Stable(pool) => {
                    if let Some(boundary_pool) =
                        boundary::liquidity::stable::to_boundary_pool(liquidity.address, pool)
                    {
                        for pair in pool.reserves.token_pairs() {
                            let token_pair = to_boundary_token_pair(&pair);
                            amms.entry(token_pair).or_default().push(Amm {
                                id: liquidity.id.clone(),
                                token_pair,
                                pool: Pool::Stable(boundary_pool.clone()),
                            });
                        }
                    }
                }
                // The baseline solver does not currently support other AMMs.
                _ => {}
            };
            amms
        })
}

#[derive(Debug)]
struct Amm {
    id: liquidity::Id,
    token_pair: TokenPair,
    pool: Pool,
}

#[derive(Debug)]
enum Pool {
    ConstantProduct(boundary::liquidity::constant_product::Pool),
    WeightedProduct(boundary::liquidity::weighted_product::Pool),
    Stable(boundary::liquidity::stable::Pool),
}

impl BaselineSolvable for Amm {
    fn get_amount_out(&self, out_token: H160, input: (U256, H160)) -> Option<U256> {
        match &self.pool {
            Pool::ConstantProduct(pool) => pool.get_amount_out(out_token, input),
            Pool::WeightedProduct(pool) => pool.get_amount_out(out_token, input),
            Pool::Stable(pool) => pool.get_amount_out(out_token, input),
        }
    }

    fn get_amount_in(&self, in_token: H160, out: (U256, H160)) -> Option<U256> {
        match &self.pool {
            Pool::ConstantProduct(pool) => pool.get_amount_in(in_token, out),
            Pool::WeightedProduct(pool) => pool.get_amount_in(in_token, out),
            Pool::Stable(pool) => pool.get_amount_in(in_token, out),
        }
    }

    fn gas_cost(&self) -> usize {
        match &self.pool {
            Pool::ConstantProduct(pool) => pool.gas_cost(),
            Pool::WeightedProduct(pool) => pool.gas_cost(),
            Pool::Stable(pool) => pool.gas_cost(),
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
