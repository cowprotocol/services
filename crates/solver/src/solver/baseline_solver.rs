use {
    crate::{
        liquidity::{
            slippage::{SlippageCalculator, SlippageContext},
            token_pairs,
            AmmOrderExecution,
            ConstantProductOrder,
            LimitOrder,
            LimitOrderExecution,
            Liquidity,
            WeightedProductOrder,
        },
        settlement::Settlement,
        solver::{Auction, Solver},
    },
    anyhow::Result,
    ethcontract::{Account, H160, U256},
    maplit::hashmap,
    model::TokenPair,
    shared::{
        baseline_solver::{
            estimate_buy_amount,
            estimate_sell_amount,
            BaseTokens,
            BaselineSolvable,
        },
        http_solver::model::TokenAmount,
        sources::{balancer_v2::swap::WeightedPoolRef, uniswap_v2::pool_fetching::Pool},
    },
    std::{collections::HashMap, sync::Arc},
};

pub struct BaselineSolver {
    account: Account,
    base_tokens: Arc<BaseTokens>,
    slippage_calculator: SlippageCalculator,
}

#[async_trait::async_trait]
impl Solver for BaselineSolver {
    async fn solve(
        &self,
        Auction {
            orders,
            liquidity,
            external_prices,
            ..
        }: Auction,
    ) -> Result<Vec<Settlement>> {
        let slippage = self.slippage_calculator.context(&external_prices);
        Ok(self.solve_(orders, liquidity, slippage))
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
    pub fn new(
        account: Account,
        base_tokens: Arc<BaseTokens>,
        slippage_calculator: SlippageCalculator,
    ) -> Self {
        Self {
            account,
            base_tokens,
            slippage_calculator,
        }
    }

    fn solve_(
        &self,
        mut limit_orders: Vec<LimitOrder>,
        liquidity: Vec<Liquidity>,
        slippage: SlippageContext,
    ) -> Vec<Settlement> {
        limit_orders.retain(|order| !order.is_liquidity_order());
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
                            // TODO - https://github.com/cowprotocol/services/issues/80
                            tracing::debug!("Excluded stable pool from baseline solving.")
                        }
                        Liquidity::LimitOrder(_) => {}
                        Liquidity::Concentrated(_) => {} /* not being implemented right now since
                                                          * baseline solver
                                                          * is not winning anyway */
                    }
                    amm_map
                });

        // We assume that individual settlements do not move the amm pools significantly
        // when returning multiple settlements.
        let mut settlements = Vec::new();

        // Return a solution for the first settle-able user order
        for order in user_orders {
            let solution = match self.settle_order(&order, &amm_map) {
                Some(solution) => solution,
                None => continue,
            };

            match solution.into_settlement(&order, &slippage) {
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
        self.solve_(orders, liquidity, SlippageContext::default())
            .into_iter()
            .next()
            .unwrap()
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
    fn into_settlement(self, order: &LimitOrder, slippage: &SlippageContext) -> Result<Settlement> {
        let mut settlement = Settlement::new(hashmap! {
            order.sell_token => self.executed_buy_amount,
            order.buy_token => self.executed_sell_amount,
        });

        let execution = LimitOrderExecution {
            filled: order.full_execution_amount(),
            // TODO: We still need to compute a `solver_fee` for partially fillable limit orders.
            solver_fee: order.solver_fee,
        };
        settlement.with_liquidity(order, execution)?;

        let (mut sell_amount, mut sell_token) = (self.executed_sell_amount, order.sell_token);
        for amm in self.path {
            let buy_token = amm.tokens.other(&sell_token).expect("Inconsistent path");
            let buy_amount = amm
                .get_amount_out(buy_token, (sell_amount, sell_token))
                .expect("Path was found, so amount must be calculable");
            let execution = slippage.apply_to_amm_execution(AmmOrderExecution {
                input_max: TokenAmount::new(sell_token, sell_amount),
                output: TokenAmount::new(buy_token, buy_amount),
                internalizable: false,
            })?;
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
        address: amm.address,
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
    use {
        super::*,
        crate::{
            liquidity::{
                tests::CapturingSettlementHandler,
                AmmOrderExecution,
                ConstantProductOrder,
                LimitOrder,
            },
            test::account,
        },
        model::order::OrderKind,
        num::rational::Ratio,
        shared::{
            addr,
            sources::balancer_v2::{
                pool_fetching::{TokenState, WeightedTokenState},
                swap::fixed_point::Bfp,
            },
        },
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
                id: 0.into(),
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
                id: 1.into(),
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
                address: H160::from_low_u64_be(1),
                tokens: TokenPair::new(buy_token, sell_token).unwrap(),
                reserves: (1_000_000, 1_000_000),
                fee: Ratio::new(3, 1000),
                settlement_handling: amm_handler[0].clone(),
            },
            // Path via native token has more liquidity
            ConstantProductOrder {
                address: H160::from_low_u64_be(2),
                tokens: TokenPair::new(sell_token, native_token).unwrap(),
                reserves: (10_000_000, 10_000_000),
                fee: Ratio::new(3, 1000),
                settlement_handling: amm_handler[1].clone(),
            },
            // Second native token pool has a worse price despite larger k
            ConstantProductOrder {
                address: H160::from_low_u64_be(3),
                tokens: TokenPair::new(sell_token, native_token).unwrap(),
                reserves: (11_000_000, 10_000_000),
                fee: Ratio::new(3, 1000),
                settlement_handling: amm_handler[1].clone(),
            },
            ConstantProductOrder {
                address: H160::from_low_u64_be(4),
                tokens: TokenPair::new(native_token, buy_token).unwrap(),
                reserves: (10_000_000, 10_000_000),
                fee: Ratio::new(3, 1000),
                settlement_handling: amm_handler[2].clone(),
            },
        ];
        let liquidity = amms.into_iter().map(Liquidity::ConstantProduct).collect();

        let base_tokens = Arc::new(BaseTokens::new(native_token, &[]));
        let solver = BaselineSolver::new(account(), base_tokens, SlippageCalculator::default());
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
        assert_eq!(
            order_handler[1].clone().calls()[0],
            LimitOrderExecution::new(100_000.into(), 0.into())
        );

        // Second & Third AMM are matched with slippage applied
        let slippage = SlippageContext::default();
        assert_eq!(amm_handler[0].clone().calls().len(), 0);
        assert_eq!(
            amm_handler[1].clone().calls()[0],
            slippage
                .apply_to_amm_execution(AmmOrderExecution {
                    input_max: TokenAmount::new(sell_token, 100_000),
                    output: TokenAmount::new(native_token, 98_715),
                    internalizable: false
                })
                .unwrap(),
        );
        assert_eq!(
            amm_handler[2].clone().calls()[0],
            slippage
                .apply_to_amm_execution(AmmOrderExecution {
                    input_max: TokenAmount::new(native_token, 98_715),
                    output: TokenAmount::new(buy_token, 97_459),
                    internalizable: false
                })
                .unwrap(),
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
                id: 0.into(),
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
                id: 1.into(),
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
                address: H160::from_low_u64_be(1),
                tokens: TokenPair::new(buy_token, sell_token).unwrap(),
                reserves: (1_000_000, 1_000_000),
                fee: Ratio::new(3, 1000),
                settlement_handling: amm_handler[0].clone(),
            },
            // Path via native token has more liquidity
            ConstantProductOrder {
                address: H160::from_low_u64_be(2),
                tokens: TokenPair::new(sell_token, native_token).unwrap(),
                reserves: (10_000_000, 10_000_000),
                fee: Ratio::new(3, 1000),
                settlement_handling: amm_handler[1].clone(),
            },
            // Second native token pool has a worse price despite larger k
            ConstantProductOrder {
                address: H160::from_low_u64_be(3),
                tokens: TokenPair::new(sell_token, native_token).unwrap(),
                reserves: (11_000_000, 10_000_000),
                fee: Ratio::new(3, 1000),
                settlement_handling: amm_handler[1].clone(),
            },
            ConstantProductOrder {
                address: H160::from_low_u64_be(4),
                tokens: TokenPair::new(native_token, buy_token).unwrap(),
                reserves: (10_000_000, 10_000_000),
                fee: Ratio::new(3, 1000),
                settlement_handling: amm_handler[2].clone(),
            },
        ];
        let liquidity = amms.into_iter().map(Liquidity::ConstantProduct).collect();

        let base_tokens = Arc::new(BaseTokens::new(native_token, &[]));
        let solver = BaselineSolver::new(account(), base_tokens, SlippageCalculator::default());
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
        assert_eq!(
            order_handler[1].clone().calls()[0],
            LimitOrderExecution::new(100_000.into(), 0.into())
        );

        // Second & Third AMM are matched with slippage applied
        let slippage = SlippageContext::default();
        assert_eq!(amm_handler[0].clone().calls().len(), 0);
        assert_eq!(
            amm_handler[1].clone().calls()[0],
            slippage
                .apply_to_amm_execution(AmmOrderExecution {
                    input_max: TokenAmount::new(sell_token, 102_660),
                    output: TokenAmount::new(native_token, 101_315),
                    internalizable: false
                })
                .unwrap(),
        );
        assert_eq!(
            amm_handler[2].clone().calls()[0],
            slippage
                .apply_to_amm_execution(AmmOrderExecution {
                    input_max: TokenAmount::new(native_token, 101_315),
                    output: TokenAmount::new(buy_token, 100_000),
                    internalizable: false
                })
                .unwrap(),
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
            id: 0.into(),
            ..Default::default()
        }];

        let amms = vec![
            ConstantProductOrder {
                address: H160::from_low_u64_be(1),
                tokens: TokenPair::new(buy_token, sell_token).unwrap(),
                reserves: (10_000_000, 10_000_000),
                fee: Ratio::new(3, 1000),
                settlement_handling: CapturingSettlementHandler::arc(),
            },
            // Other direct pool has not enough liquidity to compute a valid estimate
            ConstantProductOrder {
                address: H160::from_low_u64_be(2),
                tokens: TokenPair::new(buy_token, sell_token).unwrap(),
                reserves: (0, 0),
                fee: Ratio::new(3, 1000),
                settlement_handling: CapturingSettlementHandler::arc(),
            },
        ];
        let liquidity = amms.into_iter().map(Liquidity::ConstantProduct).collect();

        let base_tokens = Arc::new(BaseTokens::new(H160::zero(), &[]));
        let solver = BaselineSolver::new(account(), base_tokens, SlippageCalculator::default());
        assert_eq!(
            solver
                .solve_(orders, liquidity, SlippageContext::default())
                .len(),
            1
        );
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
            id: 0.into(),
            ..Default::default()
        };
        let liquidity = vec![
            Liquidity::ConstantProduct(ConstantProductOrder {
                address: H160::from_low_u64_be(1),
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
                address: H160::from_low_u64_be(2),
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
        let solver = BaselineSolver::new(account(), base_tokens, SlippageCalculator::default());
        assert_eq!(
            solver
                .solve_(vec![order], liquidity, SlippageContext::default())
                .len(),
            0
        );
    }

    #[test]
    fn does_not_panic_for_asymmetrical_pool() {
        let tokens: Vec<H160> = (0..3).map(H160::from_low_u64_be).collect();
        let order = LimitOrder {
            id: 0.into(),
            sell_token: tokens[0],
            buy_token: tokens[2],
            sell_amount: 7999613.into(),
            buy_amount: 1.into(),
            kind: OrderKind::Buy,
            ..Default::default()
        };
        let pool_0 = ConstantProductOrder {
            address: H160::from_low_u64_be(1),
            tokens: TokenPair::new(tokens[1], tokens[2]).unwrap(),
            reserves: (10, 12),
            fee: Ratio::new(0, 1),
            settlement_handling: CapturingSettlementHandler::arc(),
        };
        let pool_1 = WeightedProductOrder {
            address: H160::from_low_u64_be(2),
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
        // When baseline solver goes from the buy token to the sell token it sees that a
        // path with a sell amount of 7999613.
        assert_eq!(
            pool_0.get_amount_in(tokens[1], (1.into(), tokens[2])),
            Some(1.into())
        );
        assert_eq!(
            pool_1.get_amount_in(tokens[0], (1.into(), tokens[1])),
            Some(7999613.into())
        );
        // But then when it goes from the sell token to the buy token to construct the
        // settlement it encounters the asymmetry of the weighted pool. With the
        // same in amount the out amount has changed from 1 to 0.
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
        let solver = BaselineSolver::new(account(), base_tokens, SlippageCalculator::default());
        let settlements = solver.solve_(vec![order], liquidity, Default::default());
        assert!(settlements.is_empty());
    }
}
