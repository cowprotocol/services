use crate::encoding::EncodedInteraction;
use crate::settlement::Interaction;
use crate::{
    liquidity::{AmmOrderExecution, LimitOrder, Liquidity},
    settlement::Settlement,
};
use anyhow::{anyhow, Result};
use ethcontract::Bytes;
use model::order::OrderKind;
use primitive_types::{H160, U256};
use shared::http_solver_api::model::*;
use std::collections::{hash_map::Entry, HashMap};

// To send an instance to the solver we need to identify tokens and orders through strings. This
// struct combines the created model and a mapping of those identifiers to their original value.
#[derive(Clone, Debug)]
pub struct SettlementContext {
    pub orders: Vec<LimitOrder>,
    pub liquidity: Vec<Liquidity>,
}

pub fn convert_settlement(
    settled: SettledBatchAuctionModel,
    context: SettlementContext,
) -> Result<Settlement> {
    match IntermediateSettlement::new(settled.clone(), context)
        .and_then(|intermediate| intermediate.into_settlement())
    {
        Ok(settlement) => Ok(settlement),
        Err(err) => {
            tracing::debug!("failed to process HTTP solver result: {:?}", settled);
            Err(err)
        }
    }
}

#[derive(Clone, Debug)]
#[cfg_attr(test, derive(PartialEq))]
enum Execution {
    ExecutionAmm(ExecutedAmm),
    ExecutionCustomInteraction(InteractionData),
}

impl Execution {
    fn coordinates(&self) -> Option<ExecutionPlanCoordinatesModel> {
        match self {
            Execution::ExecutionAmm(executed_amm) => executed_amm.exec_plan.clone(),
            Execution::ExecutionCustomInteraction(interaction) => interaction.exec_plan.clone(),
        }
    }
}

// An intermediate representation between SettledBatchAuctionModel and Settlement useful for doing
// the error checking up front and then working with a more convenient representation.
struct IntermediateSettlement {
    executed_limit_orders: Vec<ExecutedLimitOrder>,
    executions: Vec<Execution>, // executions are sorted by execution coordinate.
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

#[derive(Clone, Debug)]
#[cfg_attr(test, derive(PartialEq))]
struct ExecutedAmm {
    input: (H160, U256),
    output: (H160, U256),
    order: Liquidity,
    exec_plan: Option<ExecutionPlanCoordinatesModel>,
}

impl Interaction for InteractionData {
    fn encode(&self) -> Vec<EncodedInteraction> {
        vec![(self.target, self.value, Bytes(self.call_data.clone()))]
    }
}

impl IntermediateSettlement {
    fn new(settled: SettledBatchAuctionModel, context: SettlementContext) -> Result<Self> {
        let executed_limit_orders =
            match_prepared_and_settled_orders(context.orders, settled.orders)?;
        let executions_amm = match_prepared_and_settled_amms(context.liquidity, settled.amms)?;
        let executions_interactions = settled
            .interaction_data
            .into_iter()
            .map(Execution::ExecutionCustomInteraction)
            .collect();
        let executions = merge_and_order_executions(executions_amm, executions_interactions)?;
        let prices = match_settled_prices(executed_limit_orders.as_slice(), settled.prices)?;
        Ok(Self {
            executed_limit_orders,
            executions,
            prices,
        })
    }

    fn into_settlement(self) -> Result<Settlement> {
        let mut settlement = Settlement::new(self.prices);
        for order in self.executed_limit_orders.iter() {
            settlement.with_liquidity(&order.order, order.executed_amount())?;
        }
        for execution in self.executions.iter() {
            match execution {
                Execution::ExecutionAmm(executed_amm) => {
                    let execution = AmmOrderExecution {
                        input: executed_amm.input,
                        output: executed_amm.output,
                    };
                    match &executed_amm.order {
                        Liquidity::ConstantProduct(liquidity) => {
                            settlement.with_liquidity(liquidity, execution)?
                        }
                        Liquidity::BalancerWeighted(liquidity) => {
                            settlement.with_liquidity(liquidity, execution)?
                        }
                        Liquidity::BalancerStable(liquidity) => {
                            settlement.with_liquidity(liquidity, execution)?
                        }
                    };
                }
                Execution::ExecutionCustomInteraction(interaction_data) => {
                    settlement
                        .encoder
                        .append_to_execution_plan(interaction_data.clone());
                }
            }
        }
        Ok(settlement)
    }
}

fn match_prepared_and_settled_orders(
    prepared_orders: Vec<LimitOrder>,
    settled_orders: HashMap<usize, ExecutedOrderModel>,
) -> Result<Vec<ExecutedLimitOrder>> {
    settled_orders
        .into_iter()
        .filter(|(_, settled)| {
            !(settled.exec_sell_amount.is_zero() && settled.exec_buy_amount.is_zero())
        })
        .map(|(index, settled)| {
            let prepared = prepared_orders
                .get(index)
                .ok_or_else(|| anyhow!("invalid order {}", index))?;
            Ok(ExecutedLimitOrder {
                order: prepared.clone(),
                executed_buy_amount: settled.exec_buy_amount,
                executed_sell_amount: settled.exec_sell_amount,
            })
        })
        .collect()
}

fn match_prepared_and_settled_amms(
    prepared_amms: Vec<Liquidity>,
    settled_amms: HashMap<usize, UpdatedAmmModel>,
) -> Result<Vec<Execution>> {
    settled_amms
        .into_iter()
        .filter(|(_, settled)| settled.is_non_trivial())
        .flat_map(|(index, settled)| settled.execution.into_iter().map(move |exec| (index, exec)))
        .map(|(index, settled)| {
            Ok(Execution::ExecutionAmm(ExecutedAmm {
                order: prepared_amms
                    .get(index)
                    .ok_or_else(|| anyhow!("Invalid AMM {}", index))?
                    .clone(),
                input: (settled.buy_token, settled.exec_buy_amount),
                output: (settled.sell_token, settled.exec_sell_amount),
                exec_plan: settled.exec_plan,
            }))
        })
        .collect::<Result<Vec<Execution>>>()
}

fn merge_and_order_executions(
    mut executions_amms: Vec<Execution>,
    mut interactions: Vec<Execution>,
) -> Result<Vec<Execution>> {
    interactions.append(&mut executions_amms);
    // executions with optional execution plan will be executed first
    interactions.sort_by_key(|a| a.coordinates());
    Ok(interactions)
}

fn match_settled_prices(
    executed_limit_orders: &[ExecutedLimitOrder],
    solver_prices: HashMap<H160, U256>,
) -> Result<HashMap<H160, U256>> {
    let mut prices = HashMap::new();
    let executed_tokens = executed_limit_orders
        .iter()
        .flat_map(|order| vec![order.order.buy_token, order.order.sell_token]);
    for token in executed_tokens {
        if let Entry::Vacant(entry) = prices.entry(token) {
            let price = solver_prices
                .get(&token)
                .ok_or_else(|| anyhow!("invalid token {}", token))?;
            entry.insert(*price);
        }
    }
    Ok(prices)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::liquidity::{
        tests::CapturingSettlementHandler, ConstantProductOrder, StablePoolOrder,
        WeightedProductOrder,
    };
    use hex_literal::hex;
    use maplit::hashmap;
    use model::TokenPair;
    use num::rational::Ratio;
    use num::BigRational;
    use shared::sources::balancer_v2::{
        pool_fetching::AmplificationParameter,
        pool_fetching::{TokenState, WeightedTokenState},
        swap::fixed_point::Bfp,
    };

    #[test]
    fn convert_settlement_() {
        let t0 = H160::zero();
        let t1 = H160::from_low_u64_be(1);

        let limit_handler = CapturingSettlementHandler::arc();
        let orders = vec![LimitOrder {
            sell_token: t0,
            buy_token: t1,
            sell_amount: 1.into(),
            buy_amount: 2.into(),
            kind: OrderKind::Sell,
            partially_fillable: false,
            scaled_fee_amount: Default::default(),
            settlement_handling: limit_handler.clone(),
            is_liquidity_order: false,
            id: "0".to_string(),
        }];

        let cp_amm_handler = CapturingSettlementHandler::arc();
        let wp_amm_handler = CapturingSettlementHandler::arc();
        let sp_amm_handler = CapturingSettlementHandler::arc();
        let liquidity = vec![
            Liquidity::ConstantProduct(ConstantProductOrder {
                tokens: TokenPair::new(t0, t1).unwrap(),
                reserves: (3, 4),
                fee: 5.into(),
                settlement_handling: cp_amm_handler.clone(),
            }),
            Liquidity::BalancerWeighted(WeightedProductOrder {
                reserves: hashmap! {
                    t0 => WeightedTokenState {
                        common: TokenState {
                            balance: U256::from(200),
                            scaling_exponent: 4,
                        },
                        weight: Bfp::from(200_000_000_000_000_000),
                    },
                    t1 => WeightedTokenState {
                        common: TokenState {
                            balance: U256::from(800),
                            scaling_exponent: 6,
                        },
                        weight: Bfp::from(800_000_000_000_000_000),
                    }
                },
                fee: "0.03".parse().unwrap(),
                settlement_handling: wp_amm_handler.clone(),
            }),
            Liquidity::BalancerStable(StablePoolOrder {
                reserves: hashmap! {
                    t0 => TokenState {
                        balance: U256::from(300),
                        scaling_exponent: 0,
                    },
                    t1 => TokenState {
                        balance: U256::from(400),
                        scaling_exponent: 0,
                    },
                },
                fee: BigRational::new(3.into(), 1.into()),
                amplification_parameter: AmplificationParameter::new(1.into(), 1.into()).unwrap(),
                settlement_handling: sp_amm_handler.clone(),
            }),
        ];

        let executed_order = ExecutedOrderModel {
            exec_buy_amount: 6.into(),
            exec_sell_amount: 7.into(),
            cost: Default::default(),
            fee: Default::default(),
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
            cost: Default::default(),
        };
        let updated_balancer_weighted = UpdatedAmmModel {
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
            cost: Default::default(),
        };
        let updated_balancer_stable = UpdatedAmmModel {
            execution: vec![ExecutedAmmModel {
                sell_token: t1,
                buy_token: t0,
                exec_sell_amount: U256::from(6),
                exec_buy_amount: U256::from(4),
                exec_plan: Some(ExecutionPlanCoordinatesModel {
                    sequence: 2,
                    position: 0,
                }),
            }],
            cost: Default::default(),
        };
        let settled = SettledBatchAuctionModel {
            orders: hashmap! { 0 => executed_order },
            amms: hashmap! { 0 => updated_uniswap, 1 => updated_balancer_weighted, 2 => updated_balancer_stable },
            ref_token: Some(t0),
            prices: hashmap! { t0 => 10.into(), t1 => 11.into() },
            interaction_data: Vec::new(),
        };

        let prepared = SettlementContext { orders, liquidity };

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
        assert_eq!(
            sp_amm_handler.calls(),
            vec![AmmOrderExecution {
                input: (t0, 4.into()),
                output: (t1, 6.into()),
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

        let wpo = WeightedProductOrder {
            reserves: hashmap! {
                token_c => WeightedTokenState {
                    common: TokenState {
                        balance: U256::from(1251682293173877359u128),
                        scaling_exponent: 0,
                    },
                    weight: Bfp::from(500_000_000_000_000_000),
                },
                token_b => WeightedTokenState {
                    common: TokenState {
                        balance: U256::from(799086982149629058u128),
                        scaling_exponent: 0,
                    },
                    weight: Bfp::from(500_000_000_000_000_000),
                }
            },
            fee: "0.001".parse().unwrap(),
            settlement_handling: CapturingSettlementHandler::arc(),
        };

        let spo = StablePoolOrder {
            reserves: hashmap! {
                token_c => TokenState {
                    balance: U256::from(1234u128),
                    scaling_exponent: 0
                },
                token_b => TokenState {
                    balance: U256::from(5678u128),
                    scaling_exponent: 0
                },
            },
            fee: BigRational::new(1.into(), 1000.into()),
            amplification_parameter: AmplificationParameter::new(1.into(), 1.into()).unwrap(),
            settlement_handling: CapturingSettlementHandler::arc(),
        };

        let liquidity = vec![
            Liquidity::ConstantProduct(cpo_0.clone()),
            Liquidity::ConstantProduct(cpo_1.clone()),
            Liquidity::BalancerWeighted(wpo.clone()),
            Liquidity::BalancerStable(spo.clone()),
        ];
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
                },
                "3": {
                    "kind": "Stable",
                    "reserves": {
                        "0xe4b9895e638f54c3bee2a3a78d6a297cc03e0353": "1234",
                        "0xc778417e063141139fce010982780140aa0cd5ab": "5678"
                    },
                    "fee": "0.001",
                    "cost": {
                        "token": "0xc778417e063141139fce010982780140aa0cd5ab",
                        "amount": "1771"
                    },
                    "execution": [
                        {
                            "sell_token": "0xc778417e063141139fce010982780140aa0cd5ab",
                            "buy_token": "0xe4b9895e638f54c3bee2a3a78d6a297cc03e0353",
                            "exec_sell_amount": "3",
                            "exec_buy_amount": "4",
                            "exec_plan": {
                                "sequence": 0,
                                "position": 3
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

        let prepared_amms =
            match_prepared_and_settled_amms(liquidity, solution_response.amms).unwrap();
        let executions = merge_and_order_executions(prepared_amms, Vec::new()).unwrap();
        assert_eq!(
            executions,
            vec![
                Execution::ExecutionAmm(ExecutedAmm {
                    order: Liquidity::BalancerWeighted(wpo),
                    input: (token_c, U256::from(996570293625184642u128)),
                    output: (token_b, U256::from(354009510372384890u128)),
                    exec_plan: Some(ExecutionPlanCoordinatesModel {
                        sequence: 0u32,
                        position: 0u32,
                    }),
                }),
                Execution::ExecutionAmm(ExecutedAmm {
                    order: Liquidity::ConstantProduct(cpo_0),
                    input: (token_b, U256::from(354009510372389956u128)),
                    output: (token_a, U256::from(932415220613609833982u128)),
                    exec_plan: Some(ExecutionPlanCoordinatesModel {
                        sequence: 0u32,
                        position: 1u32,
                    }),
                }),
                Execution::ExecutionAmm(ExecutedAmm {
                    order: Liquidity::ConstantProduct(cpo_1),
                    input: (token_c, U256::from(2)),
                    output: (token_b, U256::from(1)),
                    exec_plan: Some(ExecutionPlanCoordinatesModel {
                        sequence: 0u32,
                        position: 2u32,
                    }),
                }),
                Execution::ExecutionAmm(ExecutedAmm {
                    order: Liquidity::BalancerStable(spo),
                    input: (token_c, U256::from(4)),
                    output: (token_b, U256::from(3)),
                    exec_plan: Some(ExecutionPlanCoordinatesModel {
                        sequence: 0u32,
                        position: 3u32,
                    }),
                }),
            ],
        );
    }

    #[test]
    fn merge_and_order_executions_() {
        let token_a = H160::from_slice(&hex!("a7d1c04faf998f9161fc9f800a99a809b84cfc9d"));
        let token_b = H160::from_slice(&hex!("c778417e063141139fce010982780140aa0cd5ab"));

        let cpo_1 = ConstantProductOrder {
            tokens: TokenPair::new(token_a, token_b).unwrap(),
            reserves: (8488677530563931705, 75408146511005299032),
            fee: Ratio::new(3, 1000),
            settlement_handling: CapturingSettlementHandler::arc(),
        };
        let executions_amms = vec![Execution::ExecutionAmm(ExecutedAmm {
            order: Liquidity::ConstantProduct(cpo_1),
            input: (token_a, U256::from(2)),
            output: (token_b, U256::from(1)),
            exec_plan: None,
        })];
        let interactions = vec![Execution::ExecutionCustomInteraction(InteractionData {
            target: H160::zero(),
            value: U256::zero(),
            call_data: Vec::new(),
            exec_plan: Some(ExecutionPlanCoordinatesModel {
                sequence: 1u32,
                position: 1u32,
            }),
        })];
        let merged_executions =
            merge_and_order_executions(executions_amms.clone(), interactions.clone()).unwrap();
        assert_eq!(
            merged_executions,
            vec![
                executions_amms.get(0).unwrap().clone(),
                interactions.get(0).unwrap().clone()
            ]
        );
    }
}
