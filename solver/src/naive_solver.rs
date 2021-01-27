mod multi_order_solver;
mod single_pair_settlement;

use crate::{
    liquidity::{AmmOrder, LimitOrder, Liquidity},
    settlement::Settlement,
    solver::Solver,
};
use anyhow::Result;
use contracts::{GPv2Settlement, UniswapV2Factory, UniswapV2Router02};
use model::TokenPair;
use std::collections::HashMap;

pub struct NaiveSolver {
    pub uniswap_router: UniswapV2Router02,
    pub uniswap_factory: UniswapV2Factory,
    pub gpv2_settlement: GPv2Settlement,
}

#[async_trait::async_trait]
impl Solver for NaiveSolver {
    async fn solve(&self, liquidity: Vec<Liquidity>) -> Result<Option<Settlement>> {
        let mut limit_orders = Vec::new();
        let mut uniswaps = HashMap::new();
        for liquidity in liquidity {
            match liquidity {
                Liquidity::Limit(order) => limit_orders.push(order),
                Liquidity::Amm(uniswap) => {
                    let pair = uniswap.tokens;
                    uniswaps.insert(
                        TokenPair::new(pair.get().0, pair.get().1).expect("Invalid Pair"),
                        uniswap,
                    );
                }
            }
        }
        Ok(settle(limit_orders.into_iter(), uniswaps).await)
    }
}

async fn settle(
    orders: impl Iterator<Item = LimitOrder>,
    uniswaps: HashMap<TokenPair, AmmOrder>,
) -> Option<Settlement> {
    let orders = organize_orders_by_token_pair(orders);
    // TODO: Settle multiple token pairs in one settlement.
    for (pair, orders) in orders {
        if let Some(settlement) = settle_pair(pair, orders, &uniswaps).await {
            return Some(settlement);
        }
    }
    None
}

async fn settle_pair(
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
    Some(multi_order_solver::solve(orders.into_iter(), &uniswap))
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
