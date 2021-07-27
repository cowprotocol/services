use super::model::*;
use crate::liquidity::WeightedProductOrder;
use crate::{
    liquidity::{AmmOrderExecution, ConstantProductOrder, LimitOrder},
    settlement::Settlement,
};
use anyhow::{anyhow, ensure, Result};
use itertools::Itertools;
use model::order::OrderKind;
use primitive_types::{H160, U256};
use std::collections::{hash_map::Entry, HashMap};

// To send an instance to the solver we need to identify tokens and orders through strings. This
// struct combines the created model and a mapping of those identifiers to their original value.
#[derive(Debug)]
pub struct SettlementContext {
    pub limit_orders: HashMap<usize, LimitOrder>,
    pub constant_product_orders: HashMap<usize, ConstantProductOrder>,
    pub weighted_product_orders: HashMap<usize, WeightedProductOrder>,
}

pub fn convert_settlement(
    settled: SettledBatchAuctionModel,
    context: SettlementContext,
) -> Result<Settlement> {
    let intermediate = IntermediateSettlement::new(settled, context)?;
    intermediate.into_settlement()
}

// An intermediate representation between SettledBatchAuctionModel and Settlement useful for doing
// the error checking up front and then working with a more convenient representation.
struct IntermediateSettlement {
    executed_limit_orders: Vec<ExecutedLimitOrder>,
    executed_amms: Vec<ExecutedAmm>,
    prices: HashMap<H160, U256>,
}

struct ExecutedLimitOrder {
    order: LimitOrder,
    executed_buy_amount: U256,
    executed_sell_amount: U256,
}

impl ExecutedLimitOrder {
    fn executed_amount(&self) -> U256 {
        match self.order.kind {
            OrderKind::Buy => self.executed_buy_amount,
            OrderKind::Sell => self.executed_sell_amount,
        }
    }
}

#[derive(Clone)]
struct ExecutedAmm {
    input: (H160, U256),
    output: (H160, U256),
    order: ExecutedOrder,
}

#[derive(Clone)]
enum ExecutedOrder {
    ConstantProduct(ConstantProductOrder),
    WeightedProduct(WeightedProductOrder),
}

impl IntermediateSettlement {
    fn new(settled: SettledBatchAuctionModel, context: SettlementContext) -> Result<Self> {
        let executed_limit_orders =
            match_prepared_and_settled_orders(context.limit_orders, settled.orders)?;
        let executed_amms = match_prepared_and_settled_amms(
            context.constant_product_orders,
            context.weighted_product_orders,
            settled.amms,
        )?;
        let prices = match_settled_prices(executed_limit_orders.as_slice(), settled.prices)?;
        Ok(Self {
            executed_limit_orders,
            executed_amms,
            prices,
        })
    }

    fn into_settlement(self) -> Result<Settlement> {
        let mut settlement = Settlement::new(self.prices);
        for order in self.executed_limit_orders.iter() {
            settlement.with_liquidity(&order.order, order.executed_amount())?;
        }
        for executed_amm in self.executed_amms.iter() {
            let execution = AmmOrderExecution {
                input: executed_amm.input,
                output: executed_amm.output,
            };
            match &executed_amm.order {
                ExecutedOrder::ConstantProduct(liquidity) => {
                    settlement.with_liquidity(liquidity, execution)?
                }
                ExecutedOrder::WeightedProduct(liquidity) => {
                    settlement.with_liquidity(liquidity, execution)?
                }
            }
        }
        Ok(settlement)
    }
}

fn match_prepared_and_settled_orders(
    mut prepared_orders: HashMap<usize, LimitOrder>,
    settled_orders: HashMap<usize, ExecutedOrderModel>,
) -> Result<Vec<ExecutedLimitOrder>> {
    settled_orders
        .into_iter()
        .filter(|(_, settled)| {
            !(settled.exec_sell_amount.is_zero() && settled.exec_buy_amount.is_zero())
        })
        .map(|(index, settled)| {
            let prepared = prepared_orders
                .remove(&index)
                .ok_or_else(|| anyhow!("invalid order {}", index))?;
            Ok(ExecutedLimitOrder {
                order: prepared,
                executed_buy_amount: settled.exec_buy_amount,
                executed_sell_amount: settled.exec_sell_amount,
            })
        })
        .collect()
}

fn match_prepared_and_settled_amms(
    mut prepared_constant_product_orders: HashMap<usize, ConstantProductOrder>,
    mut prepared_weighted_product_orders: HashMap<usize, WeightedProductOrder>,
    settled_orders: HashMap<usize, UpdatedAmmModel>,
) -> Result<Vec<ExecutedAmm>> {
    let mut amm_executions = vec![];
    // Recall, prepared amm for weighted products are shifted by the constant product amms
    // We declare this outside before prepared_constant_product_orders is mutated.
    let shift = prepared_constant_product_orders.len();
    for (index, settled) in settled_orders
        .into_iter()
        .filter(|(_, settled)| settled.is_non_trivial())
        .flat_map(|(shifted_id, settled)| {
            settled
                .execution
                .into_iter()
                .map(move |exec| (shifted_id, exec))
        })
        .sorted_by(|a, b| a.1.exec_plan.cmp(&b.1.exec_plan))
    {
        let (input, output) = (
            (settled.buy_token, settled.exec_buy_amount),
            (settled.sell_token, settled.exec_sell_amount),
        );
        if index < shift && prepared_constant_product_orders.contains_key(&index) {
            amm_executions.push(ExecutedAmm {
                order: ExecutedOrder::ConstantProduct(
                    prepared_constant_product_orders.remove(&index).unwrap(),
                ),
                input,
                output,
            });
        } else if index >= shift && prepared_weighted_product_orders.contains_key(&(index - shift))
        {
            amm_executions.push(ExecutedAmm {
                order: ExecutedOrder::WeightedProduct(
                    prepared_weighted_product_orders
                        .remove(&(index - shift))
                        .unwrap(),
                ),
                input,
                output,
            });
        } else {
            return Err(anyhow!("Invalid AMM {}", index));
        }
    }
    Ok(amm_executions)
}

fn match_settled_prices(
    executed_limit_orders: &[ExecutedLimitOrder],
    solver_prices: HashMap<H160, Price>,
) -> Result<HashMap<H160, U256>> {
    let mut prices = HashMap::new();
    let executed_tokens = executed_limit_orders
        .iter()
        .flat_map(|order| vec![order.order.buy_token, order.order.sell_token]);
    for token in executed_tokens {
        if let Entry::Vacant(entry) = prices.entry(token) {
            let price = solver_prices
                .get(&token)
                .ok_or_else(|| anyhow!("invalid token {}", token))?
                .0;
            ensure!(price.is_finite() && price > 0.0, "invalid price {}", price);
            entry.insert(U256::from_f64_lossy(price));
        }
    }
    Ok(prices)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::liquidity::tests::CapturingSettlementHandler;
    use hex_literal::hex;
    use maplit::hashmap;
    use model::TokenPair;
    use num::rational::Ratio;
    use num::BigRational;
    use shared::sources::balancer::{pool_fetching::PoolTokenState, swap::fixed_point::Bfp};

    #[test]
    fn convert_settlement_() {
        let t0 = H160::zero();
        let t1 = H160::from_low_u64_be(1);

        let limit_handler = CapturingSettlementHandler::arc();
        let limit_order = LimitOrder {
            sell_token: t0,
            buy_token: t1,
            sell_amount: 1.into(),
            buy_amount: 2.into(),
            kind: OrderKind::Sell,
            partially_fillable: false,
            fee_amount: Default::default(),
            settlement_handling: limit_handler.clone(),
            id: "0".to_string(),
        };
        let orders = hashmap! { 0 => limit_order };

        let cp_amm_handler = CapturingSettlementHandler::arc();
        let constant_product_order = ConstantProductOrder {
            tokens: TokenPair::new(t0, t1).unwrap(),
            reserves: (3, 4),
            fee: 5.into(),
            settlement_handling: cp_amm_handler.clone(),
        };
        let constant_product_orders = hashmap! { 0 => constant_product_order };
        let wp_amm_handler = CapturingSettlementHandler::arc();
        let weighted_product_order = WeightedProductOrder {
            reserves: hashmap! {
                t0 => PoolTokenState {
                    balance: U256::from(200),
                    weight: Bfp::from(200_000_000_000_000_000),
                    scaling_exponent: 4,
                },
                t1 => PoolTokenState {
                    balance: U256::from(800),
                    weight: Bfp::from(800_000_000_000_000_000),
                    scaling_exponent: 6,
                }
            },
            fee: BigRational::new(3.into(), 1.into()),
            settlement_handling: wp_amm_handler.clone(),
        };
        let weighted_product_orders = hashmap! { 0 => weighted_product_order };

        let executed_order = ExecutedOrderModel {
            exec_buy_amount: 6.into(),
            exec_sell_amount: 7.into(),
        };
        let updated_uniswap = UpdatedAmmModel {
            execution: vec![ExecutedAmmModel {
                sell_token: t1,
                buy_token: t0,
                exec_sell_amount: U256::from(9),
                exec_buy_amount: U256::from(8),
                exec_plan: Some(ExecutionPlanCoordinatesModel {
                    sequence: 0,
                    position: 0,
                }),
            }],
        };

        let updated_balancer = UpdatedAmmModel {
            execution: vec![ExecutedAmmModel {
                sell_token: t1,
                buy_token: t0,
                exec_sell_amount: U256::from(2),
                exec_buy_amount: U256::from(1),
                exec_plan: Some(ExecutionPlanCoordinatesModel {
                    sequence: 1,
                    position: 0,
                }),
            }],
        };
        let settled = SettledBatchAuctionModel {
            orders: hashmap! { 0 => executed_order },
            amms: hashmap! { 0 => updated_uniswap, 1 => updated_balancer },
            ref_token: t0,
            prices: hashmap! { t0 => Price(10.0), t1 => Price(11.0) },
        };

        let prepared = SettlementContext {
            limit_orders: orders,
            constant_product_orders,
            weighted_product_orders,
        };

        let settlement = convert_settlement(settled, prepared).unwrap();
        assert_eq!(
            settlement.clearing_prices(),
            &hashmap! { t0 => 10.into(), t1 => 11.into() }
        );

        assert_eq!(limit_handler.calls(), vec![7.into()]);
        assert_eq!(
            cp_amm_handler.calls(),
            vec![AmmOrderExecution {
                input: (t0, 8.into()),
                output: (t1, 9.into()),
            }]
        );
        assert_eq!(
            wp_amm_handler.calls(),
            vec![AmmOrderExecution {
                input: (t0, 1.into()),
                output: (t1, 2.into()),
            }]
        );
    }

    #[test]
    fn match_prepared_and_settled_amms_() {
        let token_a = H160::from_slice(&hex!("a7d1c04faf998f9161fc9f800a99a809b84cfc9d"));
        let token_b = H160::from_slice(&hex!("c778417e063141139fce010982780140aa0cd5ab"));
        let token_c = H160::from_slice(&hex!("e4b9895e638f54c3bee2a3a78d6a297cc03e0353"));
        let cpo_0 = ConstantProductOrder {
            tokens: TokenPair::new(token_a, token_b).unwrap(),
            reserves: (597249810824827988770940, 225724246562756585230),
            fee: Ratio::new(3, 1000),
            settlement_handling: CapturingSettlementHandler::arc(),
        };
        let cpo_1 = ConstantProductOrder {
            tokens: TokenPair::new(token_b, token_c).unwrap(),
            reserves: (8488677530563931705, 75408146511005299032),
            fee: Ratio::new(3, 1000),
            settlement_handling: CapturingSettlementHandler::arc(),
        };
        let constant_product_orders = hashmap! { 0usize => cpo_0.clone(), 1usize => cpo_1 };
        let weighted_product_order = WeightedProductOrder {
            reserves: hashmap! {
                token_c => PoolTokenState {
                    balance: U256::from(1251682293173877359u128),
                    weight: Bfp::from(500_000_000_000_000_000),
                    scaling_exponent: 0,
                },
                token_b => PoolTokenState {
                    balance: U256::from(799086982149629058u128),
                    weight: Bfp::from(500_000_000_000_000_000),
                    scaling_exponent: 0,
                }
            },
            fee: BigRational::new(1.into(), 1000.into()),
            settlement_handling: CapturingSettlementHandler::arc(),
        };
        let weighted_product_orders = hashmap! { 0usize => weighted_product_order.clone() };

        let solution_response = serde_json::from_str::<SettledBatchAuctionModel>(
            r#"{
            "ref_token": "0xc778417e063141139fce010982780140aa0cd5ab",
            "tokens": {
                "0xa7d1c04faf998f9161fc9f800a99a809b84cfc9d": {
                    "decimals": 18,
                    "estimated_price": "377939419103409",
                    "normalize_priority": "0"
                },
                "0xc778417e063141139fce010982780140aa0cd5ab": {
                    "decimals": 18,
                    "estimated_price": "1000000000000000000",
                    "normalize_priority": "1"
                },
                "0xe4b9895e638f54c3bee2a3a78d6a297cc03e0353": {
                    "decimals": 18,
                    "estimated_price": "112874952666826941",
                    "normalize_priority": "0"
                }
            },
            "prices": {
                "0xa7d1c04faf998f9161fc9f800a99a809b84cfc9d": "379669381779741",
                "0xc778417e063141139fce010982780140aa0cd5ab": "1000000000000000000",
                "0xe4b9895e638f54c3bee2a3a78d6a297cc03e0353": "355227837551346618"
            },
            "orders": {
                "0": {
                    "sell_token": "0xe4b9895e638f54c3bee2a3a78d6a297cc03e0353",
                    "buy_token": "0xa7d1c04faf998f9161fc9f800a99a809b84cfc9d",
                    "sell_amount": "996570293625199060",
                    "buy_amount": "289046068204476404625",
                    "allow_partial_fill": false,
                    "is_sell_order": true,
                    "fee": {
                        "token": "0xe4b9895e638f54c3bee2a3a78d6a297cc03e0353",
                        "amount": "3429706374800940"
                    },
                    "cost": {
                        "token": "0xc778417e063141139fce010982780140aa0cd5ab",
                        "amount": "98173121900550"
                    },
                    "exec_sell_amount": "996570293625199060",
                    "exec_buy_amount": "932415220613609833982"
                }
            },
            "amms": {
                "0": {
                    "kind": "ConstantProduct",
                    "reserves": {
                        "0xa7d1c04faf998f9161fc9f800a99a809b84cfc9d": "597249810824827988770940",
                        "0xc778417e063141139fce010982780140aa0cd5ab": "225724246562756585230"
                    },
                    "fee": "0.003",
                    "cost": {
                        "token": "0xc778417e063141139fce010982780140aa0cd5ab",
                        "amount": "140188523735120"
                    },
                    "execution": [
                        {
                            "sell_token": "0xa7d1c04faf998f9161fc9f800a99a809b84cfc9d",
                            "buy_token": "0xc778417e063141139fce010982780140aa0cd5ab",
                            "exec_sell_amount": "932415220613609833982",
                            "exec_buy_amount": "354009510372389956",
                            "exec_plan": {
                                "sequence": 0,
                                "position": 1
                            }
                        }
                    ]
                },
                "1": {
                    "execution": [
                        {
                            "sell_token": "0xc778417e063141139fce010982780140aa0cd5ab",
                            "buy_token": "0xe4b9895e638f54c3bee2a3a78d6a297cc03e0353",
                            "exec_sell_amount": "1",
                            "exec_buy_amount": "2",
                            "exec_plan": {
                                "sequence": 0,
                                "position": 2
                            }
                        }
                    ]
                },
                "2": {
                    "kind": "WeightedProduct",
                    "reserves": {
                        "0xe4b9895e638f54c3bee2a3a78d6a297cc03e0353": {
                            "balance": "1251682293173877359",
                            "weight": "0.5"
                        },
                        "0xc778417e063141139fce010982780140aa0cd5ab": {
                            "balance": "799086982149629058",
                            "weight": "0.5"
                        }
                    },
                    "fee": "0.001",
                    "cost": {
                        "token": "0xc778417e063141139fce010982780140aa0cd5ab",
                        "amount": "177648716400000"
                    },
                    "execution": [
                        {
                            "sell_token": "0xc778417e063141139fce010982780140aa0cd5ab",
                            "buy_token": "0xe4b9895e638f54c3bee2a3a78d6a297cc03e0353",
                            "exec_sell_amount": "354009510372384890",
                            "exec_buy_amount": "996570293625184642",
                            "exec_plan": {
                                "sequence": 0,
                                "position": 0
                            }
                        }
                    ]
                }
            },
            "solver": {
                "name": "standard",
                "args": [
                    "--write_auxiliary_files",
                    "--solver",
                    "SCIP",
                    "--output_dir",
                    "/app/results"
                ],
                "runtime": 0.0,
                "runtime_preprocessing": 17.097073793411255,
                "runtime_solving": 123.31747031211853,
                "runtime_ring_finding": 0.0,
                "runtime_validation": 0.14400219917297363,
                "nr_variables": 24,
                "nr_bool_variables": 8,
                "optimality_gap": null,
                "solver_status": "ok",
                "termination_condition": "optimal",
                "exit_status": "completed"
            }
        }"#,
        )
        .unwrap();
        let matched_settlements = match_prepared_and_settled_amms(
            constant_product_orders,
            weighted_product_orders,
            solution_response.amms,
        );
        assert!(matched_settlements.is_ok());
        let prepared_amms = matched_settlements.unwrap();
        let executed_cp_order: ConstantProductOrder;
        let executed_wp_order: WeightedProductOrder;
        match prepared_amms[0].order.clone() {
            ExecutedOrder::ConstantProduct(_) => {
                panic!("Expected WeightedProductOrder!");
            }
            ExecutedOrder::WeightedProduct(order) => {
                executed_wp_order = order;
            }
        }
        match prepared_amms[1].order.clone() {
            ExecutedOrder::ConstantProduct(order) => {
                executed_cp_order = order;
            }
            ExecutedOrder::WeightedProduct(_) => {
                panic!("Expected ConstantProductOrder!")
            }
        }
        assert_eq!(executed_cp_order.tokens, cpo_0.tokens);
        assert_eq!(executed_cp_order.reserves, cpo_0.reserves);
        assert_eq!(executed_cp_order.fee, cpo_0.fee);
        assert_eq!(
            prepared_amms[1].input,
            (token_b, U256::from(354009510372389956u128))
        );
        assert_eq!(
            prepared_amms[1].output,
            (token_a, U256::from(932415220613609833982u128))
        );

        assert_eq!(executed_wp_order.reserves, weighted_product_order.reserves);
        assert_eq!(executed_wp_order.fee, weighted_product_order.fee);
        assert_eq!(
            prepared_amms[0].input,
            (token_c, U256::from(996570293625184642u128))
        );
        assert_eq!(
            prepared_amms[0].output,
            (token_b, U256::from(354009510372384890u128))
        );
    }
}
