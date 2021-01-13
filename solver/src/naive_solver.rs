mod multi_order_solver;
mod single_pair_settlement;

use self::single_pair_settlement::SinglePairSettlement;
use crate::{settlement::Settlement, solver::Solver, uniswap::Pool};
use anyhow::Result;
use contracts::{GPv2Settlement, UniswapV2Factory, UniswapV2Router02};
use model::{order::OrderCreation, TokenPair};
use std::collections::HashMap;

pub struct NaiveSolver {
    pub uniswap_router: UniswapV2Router02,
    pub uniswap_factory: UniswapV2Factory,
    pub gpv2_settlement: GPv2Settlement,
}

#[async_trait::async_trait]
impl Solver for NaiveSolver {
    async fn solve(&self, orders: Vec<model::order::Order>) -> Result<Option<Settlement>> {
        Ok(settle(
            orders.into_iter().map(|order| order.order_creation),
            &self.uniswap_router,
            &self.uniswap_factory,
            &self.gpv2_settlement,
        )
        .await)
    }
}

async fn settle(
    orders: impl Iterator<Item = OrderCreation>,
    uniswap_router: &UniswapV2Router02,
    uniswap_factory: &UniswapV2Factory,
    gpv2_settlement: &GPv2Settlement,
) -> Option<Settlement> {
    let orders = organize_orders_by_token_pair(orders);
    // TODO: Settle multiple token pairs in one settlement.
    for (pair, orders) in orders {
        if let Some(settlement) = settle_pair(pair, orders, &uniswap_factory).await {
            return Some(
                settlement.into_settlement(uniswap_router.clone(), gpv2_settlement.clone()),
            );
        }
    }
    None
}

async fn settle_pair(
    pair: TokenPair,
    orders: Vec<OrderCreation>,
    factory: &UniswapV2Factory,
) -> Option<SinglePairSettlement> {
    let pool = match Pool::from_token_pair(factory, &pair).await {
        Ok(pool) => pool,
        Err(err) => {
            tracing::warn!("Error getting AMM reserves: {}", err);
            return None;
        }
    }?;
    Some(multi_order_solver::solve(orders.into_iter(), &pool))
}

fn organize_orders_by_token_pair(
    orders: impl Iterator<Item = OrderCreation>,
) -> HashMap<TokenPair, Vec<OrderCreation>> {
    let mut result = HashMap::<_, Vec<OrderCreation>>::new();
    for (order, token_pair) in orders
        .filter(usable_order)
        .filter_map(|order| Some((order, order.token_pair()?)))
    {
        result.entry(token_pair).or_default().push(order);
    }
    result
}

fn usable_order(order: &OrderCreation) -> bool {
    !order.sell_amount.is_zero() && !order.buy_amount.is_zero()
}
