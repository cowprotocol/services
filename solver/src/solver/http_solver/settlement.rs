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
use std::{
    collections::{hash_map::Entry, HashMap},
    iter,
};

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
    executed_constant_product_amms: Vec<ExecutedConstantProductAmms>,
    executed_weighted_product_amms: Vec<ExecutedWeightedProductAmms>,
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

struct ExecutedConstantProductAmms {
    order: ConstantProductOrder,
    input: (H160, U256),
    output: (H160, U256),
}

struct ExecutedWeightedProductAmms {
    order: WeightedProductOrder,
    input: (H160, U256),
    output: (H160, U256),
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
        let prices = match_settled_prices(
            executed_limit_orders.as_slice(),
            (executed_amms.0.as_slice(), executed_amms.1.as_slice()),
            settled.prices,
        )?;
        Ok(Self {
            executed_limit_orders,
            executed_constant_product_amms: executed_amms.0,
            executed_weighted_product_amms: executed_amms.1,
            prices,
        })
    }

    fn into_settlement(self) -> Result<Settlement> {
        let mut settlement = Settlement::new(self.prices);
        for order in self.executed_limit_orders.iter() {
            settlement.with_liquidity(&order.order, order.executed_amount())?;
        }
        for amm in self.executed_constant_product_amms.iter() {
            settlement.with_liquidity(
                &amm.order,
                AmmOrderExecution {
                    input: amm.input,
                    output: amm.output,
                },
            )?;
        }
        for amm in self.executed_weighted_product_amms.iter() {
            settlement.with_liquidity(
                &amm.order,
                AmmOrderExecution {
                    input: amm.input,
                    output: amm.output,
                },
            )?;
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
) -> Result<(
    Vec<ExecutedConstantProductAmms>,
    Vec<ExecutedWeightedProductAmms>,
)> {
    let mut constant_product_executions = vec![];
    let mut weighted_product_executions = vec![];
    for (index, settled) in settled_orders
        .into_iter()
        .filter(|(_, settled)| settled.is_non_trivial())
        .flat_map(|(id, settled)| settled.execution.into_iter().map(move |exec| (id, exec)))
        .sorted_by(|a, b| a.1.exec_plan.cmp(&b.1.exec_plan))
    {
        let (input, output) = (
            (settled.buy_token, settled.exec_buy_amount),
            (settled.sell_token, settled.exec_sell_amount),
        );
        if prepared_constant_product_orders.contains_key(&index) {
            constant_product_executions.push(ExecutedConstantProductAmms {
                order: prepared_constant_product_orders.remove(&index).unwrap(),
                input,
                output,
            });
        } else if prepared_weighted_product_orders.contains_key(&index) {
            weighted_product_executions.push(ExecutedWeightedProductAmms {
                order: prepared_weighted_product_orders.remove(&index).unwrap(),
                input,
                output,
            });
        } else {
            return Err(anyhow!("Invalid AMM {}", index));
        }
    }
    Ok((constant_product_executions, weighted_product_executions))
}

fn match_settled_prices(
    executed_limit_orders: &[ExecutedLimitOrder],
    executed_amms: (
        &[ExecutedConstantProductAmms],
        &[ExecutedWeightedProductAmms],
    ),
    solver_prices: HashMap<H160, Price>,
) -> Result<HashMap<H160, U256>> {
    let mut prices = HashMap::new();
    let executed_tokens = executed_limit_orders
        .iter()
        .flat_map(|order| {
            iter::once(order.order.buy_token).chain(iter::once(order.order.sell_token))
        })
        .chain(executed_amms.0.iter().flat_map(|amm| amm.order.tokens))
        .chain(
            executed_amms
                .1
                .iter()
                .flat_map(|amm| amm.order.reserves.keys().copied().collect::<Vec<H160>>()),
        );
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
    use maplit::hashmap;
    use model::TokenPair;
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
        let weighted_product_orders = hashmap! { 1 => weighted_product_order };

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
}
