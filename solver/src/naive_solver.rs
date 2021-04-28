mod multi_order_solver;

use crate::{
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

fn extract_deepest_amm_liquidity(liquidity: &[Liquidity]) -> HashMap<TokenPair, AmmOrder> {
    let mut result = HashMap::new();
    for liquidity in liquidity {
        match liquidity {
            Liquidity::Amm(order) => {
                let deepest_so_far = result.entry(order.tokens).or_insert_with(|| order.clone());
                if deepest_so_far.constant_product() < order.constant_product() {
                    result.insert(order.tokens, order.clone());
                }
            }
            _ => continue,
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use ethcontract::H160;
    use num::rational::Ratio;

    use crate::liquidity::tests::CapturingSettlementHandler;

    use super::*;

    #[test]
    fn test_extract_deepest_amm_liquidity() {
        let token_pair =
            TokenPair::new(H160::from_low_u64_be(0), H160::from_low_u64_be(1)).unwrap();
        let unrelated_token_pair =
            TokenPair::new(H160::from_low_u64_be(2), H160::from_low_u64_be(3)).unwrap();
        let handler = CapturingSettlementHandler::arc();
        let liquidity = vec![
            // Deep pool
            AmmOrder {
                tokens: token_pair,
                reserves: (10_000_000, 10_000_000),
                fee: Ratio::new(3, 1000),
                settlement_handling: handler.clone(),
            },
            // Shallow pool
            AmmOrder {
                tokens: token_pair,
                reserves: (100, 100),
                fee: Ratio::new(3, 1000),
                settlement_handling: handler.clone(),
            },
            // unrelated pool
            AmmOrder {
                tokens: unrelated_token_pair,
                reserves: (10_000_000, 10_000_000),
                fee: Ratio::new(3, 1000),
                settlement_handling: handler,
            },
        ];
        let result = extract_deepest_amm_liquidity(
            &liquidity
                .iter()
                .cloned()
                .map(Liquidity::Amm)
                .collect::<Vec<_>>(),
        );
        assert_eq!(result[&token_pair].reserves, liquidity[0].reserves);
        assert_eq!(
            result[&unrelated_token_pair].reserves,
            liquidity[2].reserves
        );
    }
}
