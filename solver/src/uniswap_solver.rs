use anyhow::Result;
use ethcontract::{H160, U256};
use maplit::hashmap;
use model::TokenPair;
use shared::{
    uniswap_pool::Pool,
    uniswap_solver::{
        estimate_buy_amount, estimate_sell_amount, path_candidates, token_path_to_pair_path,
    },
};
use std::{
    collections::{HashMap, HashSet},
    fmt,
};

use crate::{
    liquidity::{uniswap::MAX_HOPS, AmmOrder, LimitOrder, Liquidity},
    settlement::Settlement,
    solver::Solver,
};
pub struct UniswapSolver {
    base_tokens: HashSet<H160>,
}

#[async_trait::async_trait]
impl Solver for UniswapSolver {
    async fn solve(&self, liquidity: Vec<Liquidity>) -> Result<Option<Settlement>> {
        Ok(self.solve(liquidity))
    }
}

impl fmt::Display for UniswapSolver {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "UniswapSolver")
    }
}

impl UniswapSolver {
    pub fn new(base_tokens: HashSet<H160>) -> Self {
        Self { base_tokens }
    }

    fn solve(&self, liquidity: Vec<Liquidity>) -> Option<Settlement> {
        let amm_map = liquidity
            .iter()
            .filter_map(|liquidity| match liquidity {
                Liquidity::Limit(_) => None,
                Liquidity::Amm(amm_order) => Some((amm_order.tokens, amm_order.clone())),
            })
            .collect::<HashMap<_, _>>();

        let pool_map = amm_map
            .iter()
            .map(|(key, value)| (*key, amm_to_pool(value)))
            .collect();

        // Return a solution for the first settle-able user order
        for liquidity in liquidity {
            let user_order = match liquidity {
                Liquidity::Limit(order) => order,
                Liquidity::Amm(_) => continue,
            };

            let solution = match self.settle_order(&user_order, &pool_map) {
                Some(solution) => solution,
                None => continue,
            };

            // Check limit price
            if solution.executed_buy_amount >= user_order.buy_amount
                && solution.executed_sell_amount <= user_order.sell_amount
            {
                return Some(solution.into_settlement(&user_order, amm_map));
            }
        }
        None
    }

    fn settle_order(
        &self,
        order: &LimitOrder,
        pools: &HashMap<TokenPair, Pool>,
    ) -> Option<Solution> {
        let candidates = path_candidates(
            order.sell_token,
            order.buy_token,
            &self.base_tokens,
            MAX_HOPS,
        );
        let (best, executed_sell_amount, executed_buy_amount) = match order.kind {
            model::order::OrderKind::Buy => {
                let path = candidates
                    .iter()
                    .min_by_key(|path| estimate_sell_amount(order.buy_amount, path, &pools))?;
                (
                    path,
                    estimate_sell_amount(order.buy_amount, path, &pools)?,
                    order.buy_amount,
                )
            }
            model::order::OrderKind::Sell => {
                let path = candidates
                    .iter()
                    .max_by_key(|path| estimate_buy_amount(order.sell_amount, path, &pools))?;
                (
                    path,
                    order.sell_amount,
                    estimate_buy_amount(order.sell_amount, path, &pools)?,
                )
            }
        };
        Some(Solution {
            path: token_path_to_pair_path(best)
                .iter()
                .map(|pair| {
                    pools
                        .get(pair)
                        .expect("Path was found so pool must exist")
                        .clone()
                })
                .collect(),
            executed_sell_amount,
            executed_buy_amount,
        })
    }
}

struct Solution {
    path: Vec<Pool>,
    executed_sell_amount: U256,
    executed_buy_amount: U256,
}

impl Solution {
    fn into_settlement(
        self,
        order: &LimitOrder,
        amm_map: HashMap<TokenPair, AmmOrder>,
    ) -> Settlement {
        //fully matched
        let matched_amount = match order.kind {
            model::order::OrderKind::Buy => order.buy_amount,
            model::order::OrderKind::Sell => order.sell_amount,
        };
        let (trade, mut interactions) = order.settlement_handling.settle(matched_amount);

        let (mut sell_amount, mut sell_token) = (self.executed_sell_amount, order.sell_token);
        for pool in self.path {
            let (buy_amount, buy_token) = pool
                .get_amount_out(sell_token, sell_amount)
                .expect("Path was found, so amount must be caluclatable");
            let amm = amm_map
                .get(&pool.tokens)
                .expect("Path was found so AMM must exist");
            interactions.extend(
                amm.settlement_handling
                    .settle((sell_token, sell_amount), (buy_token, buy_amount)),
            );
            sell_amount = buy_amount;
            sell_token = buy_token;
        }
        Settlement {
            clearing_prices: hashmap! {
                order.sell_token => self.executed_buy_amount,
                order.buy_token => self.executed_sell_amount,
            },
            fee_factor: U256::zero(),
            trades: trade.into_iter().collect(),
            intra_interactions: interactions,
            pre_interactions: Vec::new(),
            post_interactions: Vec::new(),
        }
    }
}

fn amm_to_pool(amm: &AmmOrder) -> Pool {
    Pool {
        tokens: amm.tokens,
        reserves: amm.reserves,
        fee: amm.fee,
    }
}

#[cfg(test)]
mod tests {
    use maplit::hashset;
    use model::order::OrderKind;
    use num::rational::Ratio;

    use crate::liquidity::{
        tests::{
            AmmSettlement, CapturingAmmSettlementHandler, CapturingLimitOrderSettlementHandler,
        },
        AmmOrder, LimitOrder,
    };

    use super::*;
    #[test]
    fn finds_best_route_sell_order() {
        let sell_token = H160::from_low_u64_be(1);
        let buy_token = H160::from_low_u64_be(0);
        let native_token = H160::from_low_u64_be(3);

        let order_handler = vec![
            CapturingLimitOrderSettlementHandler::arc(),
            CapturingLimitOrderSettlementHandler::arc(),
        ];
        let orders = vec![
            LimitOrder {
                sell_amount: 100_000.into(),
                buy_amount: 100_000.into(),
                sell_token,
                buy_token,
                kind: OrderKind::Sell,
                partially_fillable: false,
                settlement_handling: order_handler[0].clone(),
                id: "0".into(),
            },
            // Second order has a more lax limit
            LimitOrder {
                sell_amount: 100_000.into(),
                buy_amount: 90_000.into(),
                buy_token,
                sell_token,
                kind: OrderKind::Sell,
                partially_fillable: false,
                settlement_handling: order_handler[1].clone(),
                id: "1".into(),
            },
        ];

        let amm_handler = vec![
            CapturingAmmSettlementHandler::arc(),
            CapturingAmmSettlementHandler::arc(),
            CapturingAmmSettlementHandler::arc(),
        ];
        let amms = vec![
            AmmOrder {
                tokens: TokenPair::new(buy_token, sell_token).unwrap(),
                reserves: (1_000_000, 1_000_000),
                fee: Ratio::new(3, 1000),
                settlement_handling: amm_handler[0].clone(),
            },
            // Path via native token has more liquidity
            AmmOrder {
                tokens: TokenPair::new(sell_token, native_token).unwrap(),
                reserves: (10_000_000, 10_000_000),
                fee: Ratio::new(3, 1000),
                settlement_handling: amm_handler[1].clone(),
            },
            AmmOrder {
                tokens: TokenPair::new(native_token, buy_token).unwrap(),
                reserves: (10_000_000, 10_000_000),
                fee: Ratio::new(3, 1000),
                settlement_handling: amm_handler[2].clone(),
            },
        ];

        let mut liquidity: Vec<_> = orders.iter().cloned().map(Liquidity::Limit).collect();
        liquidity.extend(amms.iter().cloned().map(Liquidity::Amm));

        let solver = UniswapSolver::new(hashset! { native_token});
        let result = solver.solve(liquidity).unwrap();
        assert_eq!(
            result.clearing_prices,
            hashmap! {
                sell_token => 97_459.into(),
                buy_token => 100_000.into(),
            }
        );

        // Second order is fully matched
        assert_eq!(order_handler[0].clone().calls().len(), 0);
        assert_eq!(order_handler[1].clone().calls()[0], 100_000.into());

        // Second & Third AMM are matched
        assert_eq!(amm_handler[0].clone().calls().len(), 0);
        assert_eq!(
            amm_handler[1].clone().calls()[0],
            AmmSettlement {
                amount_in: 100_000.into(),
                amount_out: 98_715.into(),
                token_in: sell_token,
                token_out: native_token,
            }
        );
        assert_eq!(
            amm_handler[2].clone().calls()[0],
            AmmSettlement {
                amount_in: 98_715.into(),
                amount_out: 97_459.into(),
                token_in: native_token,
                token_out: buy_token,
            }
        );
    }

    #[test]
    fn finds_best_route_buy_order() {
        let sell_token = H160::from_low_u64_be(1);
        let buy_token = H160::from_low_u64_be(0);
        let native_token = H160::from_low_u64_be(3);

        let order_handler = vec![
            CapturingLimitOrderSettlementHandler::arc(),
            CapturingLimitOrderSettlementHandler::arc(),
        ];
        let orders = vec![
            LimitOrder {
                sell_amount: 100_000.into(),
                buy_amount: 100_000.into(),
                sell_token,
                buy_token,
                kind: OrderKind::Buy,
                partially_fillable: false,
                settlement_handling: order_handler[0].clone(),
                id: "0".into(),
            },
            // Second order has a more lax limit
            LimitOrder {
                sell_amount: 110_000.into(),
                buy_amount: 100_000.into(),
                buy_token,
                sell_token,
                kind: OrderKind::Buy,
                partially_fillable: false,
                settlement_handling: order_handler[1].clone(),
                id: "1".into(),
            },
        ];

        let amm_handler = vec![
            CapturingAmmSettlementHandler::arc(),
            CapturingAmmSettlementHandler::arc(),
            CapturingAmmSettlementHandler::arc(),
        ];
        let amms = vec![
            AmmOrder {
                tokens: TokenPair::new(buy_token, sell_token).unwrap(),
                reserves: (1_000_000, 1_000_000),
                fee: Ratio::new(3, 1000),
                settlement_handling: amm_handler[0].clone(),
            },
            // Path via native token has more liquidity
            AmmOrder {
                tokens: TokenPair::new(sell_token, native_token).unwrap(),
                reserves: (10_000_000, 10_000_000),
                fee: Ratio::new(3, 1000),
                settlement_handling: amm_handler[1].clone(),
            },
            AmmOrder {
                tokens: TokenPair::new(native_token, buy_token).unwrap(),
                reserves: (10_000_000, 10_000_000),
                fee: Ratio::new(3, 1000),
                settlement_handling: amm_handler[2].clone(),
            },
        ];

        let mut liquidity: Vec<_> = orders.iter().cloned().map(Liquidity::Limit).collect();
        liquidity.extend(amms.iter().cloned().map(Liquidity::Amm));

        let solver = UniswapSolver::new(hashset! { native_token});
        let result = solver.solve(liquidity).unwrap();
        assert_eq!(
            result.clearing_prices,
            hashmap! {
                sell_token => 100_000.into(),
                buy_token => 102_660.into(),
            }
        );

        // Second order is fully matched
        assert_eq!(order_handler[0].clone().calls().len(), 0);
        assert_eq!(order_handler[1].clone().calls()[0], 100_000.into());

        // Second & Third AMM are matched
        assert_eq!(amm_handler[0].clone().calls().len(), 0);
        assert_eq!(
            amm_handler[1].clone().calls()[0],
            AmmSettlement {
                amount_in: 102_660.into(),
                amount_out: 101_315.into(),
                token_in: sell_token,
                token_out: native_token,
            }
        );
        assert_eq!(
            amm_handler[2].clone().calls()[0],
            AmmSettlement {
                amount_in: 101_315.into(),
                amount_out: 100_000.into(),
                token_in: native_token,
                token_out: buy_token,
            }
        );
    }
}
