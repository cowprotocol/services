//! Boundary wrappers around the [`shared`] Baseline solving logic.

use crate::{
    boundary,
    domain::{eth, liquidity},
};
use ethereum_types::{H160, U256};
use shared::baseline_solver::BaselineSolvable;
use std::collections::{HashMap, HashSet};

pub use shared::baseline_solver::BaseTokens;

pub struct Solver {
    base_tokens: BaseTokens,
    liquidity: Vec<Liquidity>,
    reference_liquidity: HashMap<liquidity::Id, liquidity::Liquidity>,
}

impl Solver {
    pub fn new(
        weth: &eth::WethAddress,
        base_tokens: &HashSet<eth::TokenAddress>,
        liquidity: &[liquidity::Liquidity],
    ) -> Self {
        Self {
            base_tokens: to_boundary_base_tokens(weth, base_tokens),
            liquidity: to_boundary_liquidity(liquidity),
            reference_liquidity: liquidity
                .iter()
                .map(|liquidity| (liquidity.id, liquidity.clone()))
                .collect(),
        }
    }
}

fn to_boundary_liquidity(liquidity: &[liquidity::Liquidity]) -> Vec<Liquidity> {
    liquidity
        .iter()
        .filter_map(|liquidity| {
            let pool = match &liquidity.state {
                liquidity::State::ConstantProduct(pool) => Pool::ConstantProduct(
                    boundary::liquidity::constantproduct::to_boundary_pool(liquidity.address, pool),
                ),
                liquidity::State::WeightedProduct(pool) => Pool::Weighted(
                    boundary::liquidity::weighted::to_boundary_pool(liquidity.address, pool),
                ),
                // Other liquidity types are not supported...
                _ => return None,
            };
            Some(Liquidity {
                id: liquidity.id,
                pool,
            })
        })
        .collect()
}

struct Liquidity {
    id: liquidity::Id,
    pool: Pool,
}

enum Pool {
    ConstantProduct(boundary::liquidity::constantproduct::Pool),
    Weighted(boundary::liquidity::weighted::Pool),
}

impl BaselineSolvable for Liquidity {
    fn get_amount_out(&self, out_token: H160, input: (U256, H160)) -> Option<U256> {
        match &self.pool {
            Pool::ConstantProduct(pool) => pool.get_amount_out(out_token, input),
            Pool::Weighted(pool) => pool.get_amount_out(out_token, input),
        }
    }

    fn get_amount_in(&self, in_token: H160, out: (U256, H160)) -> Option<U256> {
        match &self.pool {
            Pool::ConstantProduct(pool) => pool.get_amount_in(in_token, out),
            Pool::Weighted(pool) => pool.get_amount_in(in_token, out),
        }
    }

    fn gas_cost(&self) -> usize {
        match &self.pool {
            Pool::ConstantProduct(pool) => pool.gas_cost(),
            Pool::Weighted(pool) => pool.gas_cost(),
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
