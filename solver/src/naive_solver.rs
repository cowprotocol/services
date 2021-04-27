mod multi_order_solver;

use crate::{
    baseline_solver::extract_deepest_amm_liquidity,
    liquidity::{AmmOrder, LimitOrder, Liquidity},
    settlement::Settlement,
    solver::Solver,
};
use anyhow::Result;
use model::TokenPair;
use std::collections::HashMap;

pub struct NaiveSolver;

#[async_trait::async_trait]
impl Solver for NaiveSolver {
    async fn solve(&self, liquidity: Vec<Liquidity>, _gas_price: f64) -> Result<Vec<Settlement>> {
        let uniswaps = extract_deepest_amm_liquidity(&liquidity);
        let limit_orders = liquidity
            .into_iter()
            .filter_map(|liquidity| match liquidity {
                Liquidity::Limit(order) => Some(order),
                _ => None,
            });
        Ok(settle(limit_orders, uniswaps).await)
    }

    fn name(&self) -> &'static str {
        "NaiveSolver"
    }
}

async fn settle(
    orders: impl Iterator<Item = LimitOrder>,
    uniswaps: HashMap<TokenPair, AmmOrder>,
) -> Vec<Settlement> {
    // The multi order solver matches as many orders as possible together with one uniswap pool.
    // Settlements between different token pairs are thus independent.
    organize_orders_by_token_pair(orders)
        .into_iter()
        .filter_map(|(pair, orders)| settle_pair(pair, orders, &uniswaps))
        .collect()
}

fn settle_pair(
    pair: TokenPair,
    orders: Vec<LimitOrder>,
    uniswaps: &HashMap<TokenPair, AmmOrder>,
) -> Option<Settlement> {
    let uniswap = match uniswaps.get(&pair) {
        Some(uniswap) => uniswap,
        None => {
            tracing::warn!("No AMM for: {:?}", pair);
            return None;
        }
    };
    multi_order_solver::solve(orders.into_iter(), &uniswap)
}

fn organize_orders_by_token_pair(
    orders: impl Iterator<Item = LimitOrder>,
) -> HashMap<TokenPair, Vec<LimitOrder>> {
    let mut result = HashMap::<_, Vec<LimitOrder>>::new();
    for (order, token_pair) in orders.filter(usable_order).filter_map(|order| {
        let pair = TokenPair::new(order.buy_token, order.sell_token)?;
        Some((order, pair))
    }) {
        result.entry(token_pair).or_default().push(order);
    }
    result
}

fn usable_order(order: &LimitOrder) -> bool {
    !order.sell_amount.is_zero() && !order.buy_amount.is_zero()
}
