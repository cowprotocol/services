use crate::{
    liquidity::{
        token_pairs, AmmOrderExecution, ConstantProductOrder, LimitOrder, Liquidity,
        WeightedProductOrder,
    },
    settlement::Settlement,
    solver::{Auction, Solver},
};
use anyhow::Result;
use ethcontract::{Account, H160, U256};
use maplit::hashmap;
use model::TokenPair;
use shared::{
    baseline_solver::{estimate_buy_amount, estimate_sell_amount, BaseTokens, BaselineSolvable},
    sources::{balancer_v2::swap::WeightedPoolRef, uniswap_v2::pool_fetching::Pool},
};
use std::{collections::HashMap, sync::Arc};

pub struct BaselineSolver {
    account: Account,
    base_tokens: Arc<BaseTokens>,
}

#[async_trait::async_trait]
impl Solver for BaselineSolver {
    async fn solve(
        &self,
        Auction {
            orders, liquidity, ..
        }: Auction,
    ) -> Result<Vec<Settlement>> {
        Ok(self.solve_(orders, liquidity))
    }

    fn account(&self) -> &Account {
        &self.account
    }

    fn name(&self) -> &'static str {
        "BaselineSolver"
    }
}

/// A type representing all possible AMM orders that are considered as on-chain
/// liquidity by the baseline solver.
#[derive(Debug, Clone)]
struct Amm {
    tokens: TokenPair,
    order: AmmOrder,
}

#[derive(Debug, Clone)]
enum AmmOrder {
    ConstantProduct(ConstantProductOrder),
    WeightedProduct(WeightedProductOrder),
}

impl BaselineSolvable for ConstantProductOrder {
    fn get_amount_out(&self, out_token: H160, input: (U256, H160)) -> Option<U256> {
        amm_to_pool(self).get_amount_out(out_token, input)
    }

    fn get_amount_in(&self, in_token: H160, output: (U256, H160)) -> Option<U256> {
        amm_to_pool(self).get_amount_in(in_token, output)
    }

    fn gas_cost(&self) -> usize {
        amm_to_pool(self).gas_cost()
    }
}

impl BaselineSolvable for WeightedProductOrder {
    fn get_amount_out(&self, out_token: H160, input: (U256, H160)) -> Option<U256> {
        amm_to_weighted_pool(self).get_amount_out(out_token, input)
    }

    fn get_amount_in(&self, in_token: H160, output: (U256, H160)) -> Option<U256> {
        amm_to_weighted_pool(self).get_amount_in(in_token, output)
    }

    fn gas_cost(&self) -> usize {
        amm_to_weighted_pool(self).gas_cost()
    }
}

impl BaselineSolvable for Amm {
    fn get_amount_out(&self, out_token: H160, input: (U256, H160)) -> Option<U256> {
        match &self.order {
            AmmOrder::ConstantProduct(order) => order.get_amount_out(out_token, input),
            AmmOrder::WeightedProduct(order) => order.get_amount_out(out_token, input),
        }
    }

    fn get_amount_in(&self, in_token: H160, output: (U256, H160)) -> Option<U256> {
        match &self.order {
            AmmOrder::ConstantProduct(order) => order.get_amount_in(in_token, output),
            AmmOrder::WeightedProduct(order) => order.get_amount_in(in_token, output),
        }
    }

    fn gas_cost(&self) -> usize {
        match &self.order {
            AmmOrder::ConstantProduct(order) => order.gas_cost(),
            AmmOrder::WeightedProduct(order) => order.gas_cost(),
        }
    }
}

impl BaselineSolver {
    pub fn new(account: Account, base_tokens: Arc<BaseTokens>) -> Self {
        Self {
            account,
            base_tokens,
        }
    }

    fn solve_(
        &self,
        mut limit_orders: Vec<LimitOrder>,
        liquidity: Vec<Liquidity>,
    ) -> Vec<Settlement> {
        limit_orders.retain(|order| !order.is_liquidity_order);
        let user_orders = limit_orders;
        let amm_map =
            liquidity
                .into_iter()
                .fold(HashMap::<_, Vec<_>>::new(), |mut amm_map, liquidity| {
                    match liquidity {
                        Liquidity::ConstantProduct(order) => {
                            amm_map.entry(order.tokens).or_default().push(Amm {
                                tokens: order.tokens,
                                order: AmmOrder::ConstantProduct(order),
                            });
                        }
                        Liquidity::BalancerWeighted(order) => {
                            for tokens in token_pairs(&order.reserves) {
                                amm_map.entry(tokens).or_default().push(Amm {
                                    tokens,
                                    order: AmmOrder::WeightedProduct(order.clone()),
                                });
                            }
                        }
                        Liquidity::BalancerStable(_order) => {
                            // TODO - https://github.com/gnosis/gp-v2-services/issues/1074
                            tracing::debug!("Excluded stable pool from baseline solving.")
                        }
                        Liquidity::LimitOrder(_) => {}
                    }
                    amm_map
                });

        // We assume that individual settlements do not move the amm pools significantly when
        // returning multiple settlements.
        let mut settlements = Vec::new();

        // Return a solution for the first settle-able user order
        for order in user_orders {
            let solution = match self.settle_order(&order, &amm_map) {
                Some(solution) => solution,
                None => continue,
            };

            match solution.into_settlement(&order) {
                Ok(settlement) => settlements.push(settlement),
                Err(err) => {
                    tracing::error!("baseline_solver failed to create settlement: {:?}", err)
                }
            }
        }

        settlements
    }

    fn settle_order(
        &self,
        order: &LimitOrder,
        amms: &HashMap<TokenPair, Vec<Amm>>,
    ) -> Option<Solution> {
        let candidates = self
            .base_tokens
            .path_candidates(order.sell_token, order.buy_token);

        let (path, executed_sell_amount, executed_buy_amount) = match order.kind {
            model::order::OrderKind::Buy => {
                let best = candidates
                    .iter()
                    .filter_map(|path| estimate_sell_amount(order.buy_amount, path, amms))
                    .filter(|estimate| estimate.value <= order.sell_amount)
                    // For buy orders we find the best path starting at the buy token ending at the
                    // sell token. When we turn this into a settlement however we need to go from
                    // the sell token to the buy token. This reversing of the direction can fail or
                    // yield different amounts as explained in the BaselineSolvable trait.
                    .filter(|estimate| {
                        matches!(
                            traverse_path_forward(
                                order.sell_token,
                                estimate.value,
                                &estimate.path,
                            ), Some(amount) if amount >= order.buy_amount
                        )
                    })
                    .min_by_key(|estimate| estimate.value)?;
                (best.path, best.value, order.buy_amount)
            }
            model::order::OrderKind::Sell => {
                let best = candidates
                    .iter()
                    .filter_map(|path| estimate_buy_amount(order.sell_amount, path, amms))
                    .filter(|estimate| estimate.value >= order.buy_amount)
                    .max_by_key(|estimate| estimate.value)?;
                (best.path, order.sell_amount, best.value)
            }
        };
        Some(Solution {
            path: path.into_iter().cloned().collect(),
            executed_sell_amount,
            executed_buy_amount,
        })
    }

    #[cfg(test)]
    fn must_solve(&self, orders: Vec<LimitOrder>, liquidity: Vec<Liquidity>) -> Settlement {
        self.solve_(orders, liquidity).into_iter().next().unwrap()
    }
}

fn traverse_path_forward(
    mut sell_token: H160,
    mut sell_amount: U256,
    path: &[&Amm],
) -> Option<U256> {
    for amm in path {
        let buy_token = amm.tokens.other(&sell_token).expect("Inconsistent path");
        let buy_amount = amm.get_amount_out(buy_token, (sell_amount, sell_token))?;
        sell_token = buy_token;
        sell_amount = buy_amount;
    }
    Some(sell_amount)
}

#[derive(Debug)]
struct Solution {
    path: Vec<Amm>,
    executed_sell_amount: U256,
    executed_buy_amount: U256,
}

impl Solution {
    fn into_settlement(self, order: &LimitOrder) -> Result<Settlement> {
        let mut settlement = Settlement::new(hashmap! {
            order.sell_token => self.executed_buy_amount,
            order.buy_token => self.executed_sell_amount,
        });

        settlement.with_liquidity(order, order.full_execution_amount())?;

        let (mut sell_amount, mut sell_token) = (self.executed_sell_amount, order.sell_token);
        for amm in self.path {
            let buy_token = amm.tokens.other(&sell_token).expect("Inconsistent path");
            let buy_amount = amm
                .get_amount_out(buy_token, (sell_amount, sell_token))
                .expect("Path was found, so amount must be calculable");
            let execution = AmmOrderExecution {
                input: (sell_token, sell_amount),
                output: (buy_token, buy_amount),
            };
            match &amm.order {
                AmmOrder::ConstantProduct(order) => settlement.with_liquidity(order, execution),
                AmmOrder::WeightedProduct(order) => settlement.with_liquidity(order, execution),
            }?;
            sell_amount = buy_amount;
            sell_token = buy_token;
        }

        Ok(settlement)
    }
}

fn amm_to_pool(amm: &ConstantProductOrder) -> Pool {
    Pool {
        tokens: amm.tokens,
        reserves: amm.reserves,
        fee: amm.fee,
    }
}

fn amm_to_weighted_pool(amm: &WeightedProductOrder) -> WeightedPoolRef {
    WeightedPoolRef {
        reserves: &amm.reserves,
        swap_fee: amm.fee,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::liquidity::{
        tests::CapturingSettlementHandler, AmmOrderExecution, ConstantProductOrder, LimitOrder,
    };
    use crate::test::account;
    use model::order::OrderKind;
    use num::rational::Ratio;
    use shared::sources::balancer_v2::swap::fixed_point::Bfp;
    use shared::{
        addr,
        sources::balancer_v2::pool_fetching::{TokenState, WeightedTokenState},
    };

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
                settlement_handling: order_handler[0].clone(),
                id: "0".into(),
                ..Default::default()
            },
            // Second order has a more lax limit
            LimitOrder {
                sell_amount: 100_000.into(),
                buy_amount: 90_000.into(),
                buy_token,
                sell_token,
                kind: OrderKind::Sell,
                settlement_handling: order_handler[1].clone(),
                id: "1".into(),
                ..Default::default()
            },
        ];

        let amm_handler = vec![
            CapturingSettlementHandler::arc(),
            CapturingSettlementHandler::arc(),
            CapturingSettlementHandler::arc(),
        ];
        let amms = vec![
            ConstantProductOrder {
                tokens: TokenPair::new(buy_token, sell_token).unwrap(),
                reserves: (1_000_000, 1_000_000),
                fee: Ratio::new(3, 1000),
                settlement_handling: amm_handler[0].clone(),
            },
            // Path via native token has more liquidity
            ConstantProductOrder {
                tokens: TokenPair::new(sell_token, native_token).unwrap(),
                reserves: (10_000_000, 10_000_000),
                fee: Ratio::new(3, 1000),
                settlement_handling: amm_handler[1].clone(),
            },
            // Second native token pool has a worse price despite larger k
            ConstantProductOrder {
                tokens: TokenPair::new(sell_token, native_token).unwrap(),
                reserves: (11_000_000, 10_000_000),
                fee: Ratio::new(3, 1000),
                settlement_handling: amm_handler[1].clone(),
            },
            ConstantProductOrder {
                tokens: TokenPair::new(native_token, buy_token).unwrap(),
                reserves: (10_000_000, 10_000_000),
                fee: Ratio::new(3, 1000),
                settlement_handling: amm_handler[2].clone(),
            },
        ];
        let liquidity = amms.into_iter().map(Liquidity::ConstantProduct).collect();

        let base_tokens = Arc::new(BaseTokens::new(native_token, &[]));
        let solver = BaselineSolver::new(account(), base_tokens);
        let result = solver.must_solve(orders, liquidity);
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
                settlement_handling: order_handler[0].clone(),
                id: "0".into(),
                ..Default::default()
            },
            // Second order has a more lax limit
            LimitOrder {
                sell_amount: 110_000.into(),
                buy_amount: 100_000.into(),
                buy_token,
                sell_token,
                kind: OrderKind::Buy,
                settlement_handling: order_handler[1].clone(),
                id: "1".into(),
                ..Default::default()
            },
        ];

        let amm_handler = vec![
            CapturingSettlementHandler::arc(),
            CapturingSettlementHandler::arc(),
            CapturingSettlementHandler::arc(),
        ];
        let amms = vec![
            ConstantProductOrder {
                tokens: TokenPair::new(buy_token, sell_token).unwrap(),
                reserves: (1_000_000, 1_000_000),
                fee: Ratio::new(3, 1000),
                settlement_handling: amm_handler[0].clone(),
            },
            // Path via native token has more liquidity
            ConstantProductOrder {
                tokens: TokenPair::new(sell_token, native_token).unwrap(),
                reserves: (10_000_000, 10_000_000),
                fee: Ratio::new(3, 1000),
                settlement_handling: amm_handler[1].clone(),
            },
            // Second native token pool has a worse price despite larger k
            ConstantProductOrder {
                tokens: TokenPair::new(sell_token, native_token).unwrap(),
                reserves: (11_000_000, 10_000_000),
                fee: Ratio::new(3, 1000),
                settlement_handling: amm_handler[1].clone(),
            },
            ConstantProductOrder {
                tokens: TokenPair::new(native_token, buy_token).unwrap(),
                reserves: (10_000_000, 10_000_000),
                fee: Ratio::new(3, 1000),
                settlement_handling: amm_handler[2].clone(),
            },
        ];
        let liquidity = amms.into_iter().map(Liquidity::ConstantProduct).collect();

        let base_tokens = Arc::new(BaseTokens::new(native_token, &[]));
        let solver = BaselineSolver::new(account(), base_tokens);
        let result = solver.must_solve(orders, liquidity);
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
    fn finds_best_route_when_pool_returns_none() {
        // Regression test for https://github.com/gnosis/gp-v2-services/issues/530
        let sell_token = H160::from_low_u64_be(1);
        let buy_token = H160::from_low_u64_be(0);

        let orders = vec![LimitOrder {
            sell_amount: 110_000.into(),
            buy_amount: 100_000.into(),
            sell_token,
            buy_token,
            kind: OrderKind::Buy,
            id: "0".into(),
            ..Default::default()
        }];

        let amms = vec![
            ConstantProductOrder {
                tokens: TokenPair::new(buy_token, sell_token).unwrap(),
                reserves: (10_000_000, 10_000_000),
                fee: Ratio::new(3, 1000),
                settlement_handling: CapturingSettlementHandler::arc(),
            },
            // Other direct pool has not enough liquidity to compute a valid estimate
            ConstantProductOrder {
                tokens: TokenPair::new(buy_token, sell_token).unwrap(),
                reserves: (0, 0),
                fee: Ratio::new(3, 1000),
                settlement_handling: CapturingSettlementHandler::arc(),
            },
        ];
        let liquidity = amms.into_iter().map(Liquidity::ConstantProduct).collect();

        let base_tokens = Arc::new(BaseTokens::new(H160::zero(), &[]));
        let solver = BaselineSolver::new(account(), base_tokens);
        assert_eq!(solver.solve_(orders, liquidity).len(), 1);
    }

    #[test]
    fn does_not_panic_when_building_solution() {
        // Regression test for https://github.com/gnosis/gp-v2-services/issues/838
        let order = LimitOrder {
            sell_token: addr!("e4b9895e638f54c3bee2a3a78d6a297cc03e0353"),
            buy_token: addr!("a7d1c04faf998f9161fc9f800a99a809b84cfc9d"),
            sell_amount: 1_741_103_528_769_588_955_u128.into(),
            buy_amount: 500_000_000_000_000_000_000_u128.into(),
            kind: OrderKind::Buy,
            id: "Crash Bandicoot".to_string(),
            ..Default::default()
        };
        let liquidity = vec![
            Liquidity::ConstantProduct(ConstantProductOrder {
                tokens: TokenPair::new(
                    addr!("a7d1c04faf998f9161fc9f800a99a809b84cfc9d"),
                    addr!("c778417e063141139fce010982780140aa0cd5ab"),
                )
                .unwrap(),
                reserves: (596_652_163_418_904_202_462_071, 225_949_669_025_168_181_644),
                fee: Ratio::new(3, 1000),
                settlement_handling: CapturingSettlementHandler::arc(),
            }),
            Liquidity::BalancerWeighted(WeightedProductOrder {
                reserves: hashmap! {
                    addr!("c778417e063141139fce010982780140aa0cd5ab") => WeightedTokenState {
                        common: TokenState {
                            balance: 799_086_982_149_629_058_u128.into(),
                            scaling_exponent: 0,
                        },
                        weight: "0.5".parse().unwrap(),
                    },
                    addr!("e4b9895e638f54c3bee2a3a78d6a297cc03e0353") => WeightedTokenState {
                        common: TokenState {
                            balance: 1_251_682_293_173_877_359_u128.into(),
                            scaling_exponent: 0,
                        },
                        weight: "0.5".parse().unwrap(),
                    },
                },
                fee: "0.001".parse().unwrap(),
                settlement_handling: CapturingSettlementHandler::arc(),
            }),
        ];

        let base_tokens = Arc::new(BaseTokens::new(
            addr!("c778417e063141139fce010982780140aa0cd5ab"),
            &[],
        ));
        let solver = BaselineSolver::new(account(), base_tokens);
        assert_eq!(solver.solve_(vec![order], liquidity).len(), 0);
    }

    #[test]
    fn does_not_panic_for_asymmetrical_pool() {
        let tokens: Vec<H160> = (0..3).map(H160::from_low_u64_be).collect();
        let order = LimitOrder {
            id: "".to_string(),
            sell_token: tokens[0],
            buy_token: tokens[2],
            sell_amount: 7999613.into(),
            buy_amount: 1.into(),
            kind: OrderKind::Buy,
            ..Default::default()
        };
        let pool_0 = ConstantProductOrder {
            tokens: TokenPair::new(tokens[1], tokens[2]).unwrap(),
            reserves: (10, 12),
            fee: Ratio::new(0, 1),
            settlement_handling: CapturingSettlementHandler::arc(),
        };
        let pool_1 = WeightedProductOrder {
            reserves: [
                (
                    tokens[0],
                    WeightedTokenState {
                        common: TokenState {
                            balance: 4294966784u64.into(),
                            scaling_exponent: 0,
                        },
                        weight: 255.into(),
                    },
                ),
                (
                    tokens[1],
                    WeightedTokenState {
                        common: TokenState {
                            balance: 4278190173u64.into(),
                            scaling_exponent: 0,
                        },
                        weight: 2030043135usize.into(),
                    },
                ),
            ]
            .iter()
            .cloned()
            .collect(),
            fee: Bfp::zero(),
            settlement_handling: CapturingSettlementHandler::arc(),
        };
        // When baseline solver goes from the buy token to the sell token it sees that a path with
        // a sell amount of 7999613.
        assert_eq!(
            pool_0.get_amount_in(tokens[1], (1.into(), tokens[2])),
            Some(1.into())
        );
        assert_eq!(
            pool_1.get_amount_in(tokens[0], (1.into(), tokens[1])),
            Some(7999613.into())
        );
        // But then when it goes from the sell token to the buy token to construct the settlement
        // it encounters the asymmetry of the weighted pool. With the same in amount the out amount
        // has changed from 1 to 0.
        assert_eq!(
            pool_1.get_amount_out(tokens[1], (7999613.into(), tokens[0])),
            Some(0.into()),
        );
        // This makes using the second pool fail.
        assert_eq!(pool_0.get_amount_in(tokens[2], (0.into(), tokens[1])), None);

        let liquidity = vec![
            Liquidity::ConstantProduct(pool_0),
            Liquidity::BalancerWeighted(pool_1),
        ];
        let base_tokens = Arc::new(BaseTokens::new(tokens[0], &tokens));
        let solver = BaselineSolver::new(account(), base_tokens);
        let settlements = solver.solve_(vec![order], liquidity);
        assert!(settlements.is_empty());
    }
}
