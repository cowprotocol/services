use anyhow::Result;
use ethcontract::{H160, U256};
use maplit::hashmap;
use model::TokenPair;
use shared::{
    pool_fetching::Pool,
    uniswap_solver::{
        estimate_buy_amount, estimate_sell_amount, path_candidates, token_path_to_pair_path,
    },
};
use std::{
    collections::{HashMap, HashSet},
    fmt,
};

use crate::{
    liquidity::{uniswap::MAX_HOPS, AmmOrder, AmmOrderExecution, LimitOrder, Liquidity},
    settlement::Settlement,
    solver::Solver,
};
pub struct UniswapSolver {
    base_tokens: HashSet<H160>,
}

#[async_trait::async_trait]
impl Solver for UniswapSolver {
    async fn solve(
        &self,
        liquidity: Vec<Liquidity>,
        _gas_price: f64,
    ) -> Result<Option<Settlement>> {
        self.solve(liquidity).transpose()
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

    fn solve(&self, liquidity: Vec<Liquidity>) -> Option<Result<Settlement>> {
        let amm_map = extract_deepest_amm_liquidity(&liquidity);

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

    #[cfg(test)]
    fn must_solve(&self, liquidity: Vec<Liquidity>) -> Settlement {
        self.solve(liquidity).unwrap().unwrap()
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
    ) -> Result<Settlement> {
        let mut settlement = Settlement::new(hashmap! {
            order.sell_token => self.executed_buy_amount,
            order.buy_token => self.executed_sell_amount,
        });

        settlement.with_liquidity(order, order.full_execution_amount())?;

        let (mut sell_amount, mut sell_token) = (self.executed_sell_amount, order.sell_token);
        for pool in self.path {
            let (buy_amount, buy_token) = pool
                .get_amount_out(sell_token, sell_amount)
                .expect("Path was found, so amount must be caluclatable");
            let amm = amm_map
                .get(&pool.tokens)
                .expect("Path was found so AMM must exist");
            settlement.with_liquidity(
                amm,
                AmmOrderExecution {
                    input: (sell_token, sell_amount),
                    output: (buy_token, buy_amount),
                },
            )?;
            sell_amount = buy_amount;
            sell_token = buy_token;
        }

        Ok(settlement)
    }
}

fn amm_to_pool(amm: &AmmOrder) -> Pool {
    Pool {
        tokens: amm.tokens,
        reserves: amm.reserves,
        fee: amm.fee,
    }
}

pub fn extract_deepest_amm_liquidity(liquidity: &[Liquidity]) -> HashMap<TokenPair, AmmOrder> {
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
    use maplit::hashset;
    use model::order::OrderKind;
    use num::rational::Ratio;

    use crate::liquidity::{
        tests::CapturingSettlementHandler, AmmOrder, AmmOrderExecution, LimitOrder,
    };

    use super::*;
    #[test]
    fn finds_best_route_sell_order() {
        let sell_token = H160::from_low_u64_be(1);
        let buy_token = H160::from_low_u64_be(0);
        let native_token = H160::from_low_u64_be(3);

        let order_handler = vec![
            CapturingSettlementHandler::arc(),
            CapturingSettlementHandler::arc(),
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
            CapturingSettlementHandler::arc(),
            CapturingSettlementHandler::arc(),
            CapturingSettlementHandler::arc(),
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
        let result = solver.must_solve(liquidity);
        assert_eq!(
            result.clearing_prices(),
            &hashmap! {
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
            AmmOrderExecution {
                input: (sell_token, 100_000.into()),
                output: (native_token, 98_715.into()),
            }
        );
        assert_eq!(
            amm_handler[2].clone().calls()[0],
            AmmOrderExecution {
                input: (native_token, 98_715.into()),
                output: (buy_token, 97_459.into()),
            }
        );
    }

    #[test]
    fn finds_best_route_buy_order() {
        let sell_token = H160::from_low_u64_be(1);
        let buy_token = H160::from_low_u64_be(0);
        let native_token = H160::from_low_u64_be(3);

        let order_handler = vec![
            CapturingSettlementHandler::arc(),
            CapturingSettlementHandler::arc(),
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
            CapturingSettlementHandler::arc(),
            CapturingSettlementHandler::arc(),
            CapturingSettlementHandler::arc(),
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
        let result = solver.must_solve(liquidity);
        assert_eq!(
            result.clearing_prices(),
            &hashmap! {
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
            AmmOrderExecution {
                input: (sell_token, 102_660.into()),
                output: (native_token, 101_315.into()),
            }
        );
        assert_eq!(
            amm_handler[2].clone().calls()[0],
            AmmOrderExecution {
                input: (native_token, 101_315.into()),
                output: (buy_token, 100_000.into()),
            }
        );
    }

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
