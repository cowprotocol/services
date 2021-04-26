use super::model::*;
use crate::{
    liquidity::{AmmOrder, AmmOrderExecution, LimitOrder},
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
pub struct SettlementContext {
    pub tokens: HashMap<String, H160>,
    pub limit_orders: HashMap<String, LimitOrder>,
    pub amm_orders: HashMap<String, AmmOrder>,
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

struct ExecutedAmm {
    order: AmmOrder,
    input: (H160, U256),
    output: (H160, U256),
}

impl IntermediateSettlement {
    fn new(settled: SettledBatchAuctionModel, context: SettlementContext) -> Result<Self> {
        let executed_limit_orders =
            match_prepared_and_settled_orders(context.limit_orders, settled.orders)?;
        let executed_amms = match_prepared_and_settled_amms(context.amm_orders, settled.uniswaps)?;
        let prices = match_settled_prices(
            &context.tokens,
            executed_limit_orders.as_slice(),
            executed_amms.as_slice(),
            settled.prices,
        )?;
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
        for amm in self.executed_amms.iter() {
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
    mut prepared_orders: HashMap<String, LimitOrder>,
    settled_orders: HashMap<String, ExecutedOrderModel>,
) -> Result<Vec<ExecutedLimitOrder>> {
    settled_orders
        .into_iter()
        .filter(|(_, settled)| {
            !(settled.exec_sell_amount.is_zero() && settled.exec_buy_amount.is_zero())
        })
        .map(|(index, settled)| {
            let prepared = prepared_orders
                .remove(index.as_str())
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
    mut prepared_orders: HashMap<String, AmmOrder>,
    settled_orders: HashMap<String, UpdatedUniswapModel>,
) -> Result<Vec<ExecutedAmm>> {
    settled_orders
        .into_iter()
        .filter(|(_, settled)| !(settled.balance_update1 == 0 && settled.balance_update2 == 0))
        .sorted_by(|a, b| a.1.exec_plan.cmp(&b.1.exec_plan))
        .map(|(index, settled)| {
            let prepared = prepared_orders
                .remove(index.as_str())
                .ok_or_else(|| anyhow!("invalid amm {}", index))?;
            let tokens = prepared.tokens.get();
            let updates = (settled.balance_update1, settled.balance_update2);
            let (input, output) = if updates.0.is_positive() && updates.1.is_negative() {
                (
                    (tokens.0, i128_abs_to_u256(updates.0)),
                    (tokens.1, i128_abs_to_u256(updates.1)),
                )
            } else if updates.1.is_positive() && updates.0.is_negative() {
                (
                    (tokens.1, i128_abs_to_u256(updates.1)),
                    (tokens.0, i128_abs_to_u256(updates.0)),
                )
            } else {
                return Err(anyhow!("invalid uniswap update {:?}", settled));
            };
            Ok(ExecutedAmm {
                order: prepared,
                input,
                output,
            })
        })
        .collect()
}

fn match_settled_prices(
    prepared_tokens: &HashMap<String, H160>,
    executed_limit_orders: &[ExecutedLimitOrder],
    executed_amms: &[ExecutedAmm],
    solver_prices: HashMap<String, Price>,
) -> Result<HashMap<H160, U256>> {
    // Remove the indirection over the token string index from the solver prices.
    let solver_prices: HashMap<H160, Price> = solver_prices
        .into_iter()
        .map(|(index, price)| {
            let token = prepared_tokens
                .get(&index)
                .ok_or_else(|| anyhow!("invalid token {}", index))?;
            Ok((*token, price))
        })
        .collect::<Result<_>>()?;

    let mut prices = HashMap::new();
    let executed_tokens = executed_limit_orders
        .iter()
        .flat_map(|order| {
            iter::once(&order.order.buy_token).chain(iter::once(&order.order.sell_token))
        })
        .chain(executed_amms.iter().flat_map(|amm| &amm.order.tokens));
    for token in executed_tokens {
        if let Entry::Vacant(entry) = prices.entry(*token) {
            let price = solver_prices
                .get(token)
                .ok_or_else(|| anyhow!("invalid token {}", token))?
                .0;
            ensure!(price.is_finite() && price > 0.0, "invalid price {}", price);
            entry.insert(U256::from_f64_lossy(price));
        }
    }
    Ok(prices)
}

fn i128_abs_to_u256(i: i128) -> U256 {
    // TODO: use `unsigned_abs` once it is stable in next compiler version
    // until then we need this check because the most negative value can not be `abs`ed because it
    // it the most positive value plus 1.
    if i == i128::MIN {
        (i128::MAX as u128 + 1).into()
    } else {
        (i.abs() as u128).into()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        encoding::EncodedInteraction,
        http_solver::model::{ExecutedOrderModel, UpdatedUniswapModel},
        liquidity::tests::CapturingSettlementHandler,
        settlement::Interaction,
    };
    use maplit::hashmap;
    use model::TokenPair;

    #[derive(Debug)]
    struct NoopInteraction;
    impl Interaction for NoopInteraction {
        fn encode(&self) -> Vec<EncodedInteraction> {
            Vec::new()
        }
    }

    #[test]
    fn convert_settlement_() {
        let t0 = H160::from_low_u64_be(0);
        let t1 = H160::from_low_u64_be(1);
        let tokens = hashmap! { "t0".to_string() => t0, "t1".to_string() => t1 };

        let limit_handler = CapturingSettlementHandler::arc();
        let limit_order = LimitOrder {
            sell_token: t0,
            buy_token: t1,
            sell_amount: 1.into(),
            buy_amount: 2.into(),
            kind: OrderKind::Sell,
            partially_fillable: false,
            settlement_handling: limit_handler.clone(),
            id: "0".to_string(),
        };
        let orders = hashmap! { "lo0".to_string() => limit_order };

        let amm_handler = CapturingSettlementHandler::arc();
        let amm_order = AmmOrder {
            tokens: TokenPair::new(t0, t1).unwrap(),
            reserves: (3, 4),
            fee: 5.into(),
            settlement_handling: amm_handler.clone(),
        };
        let amms = hashmap! { "amm0".to_string() => amm_order };

        let executed_order = ExecutedOrderModel {
            exec_buy_amount: 6.into(),
            exec_sell_amount: 7.into(),
        };
        let updated_uniswap = UpdatedUniswapModel {
            balance_update1: 8,
            balance_update2: -9,
            exec_plan: Some(ExecutionPlanCoordinatesModel {
                sequence: 0,
                position: 0,
            }),
        };
        let settled = SettledBatchAuctionModel {
            orders: hashmap! { "lo0".to_string() => executed_order },
            uniswaps: hashmap! { "amm0".to_string() => updated_uniswap },
            ref_token: "t0".to_string(),
            prices: hashmap! { "t0".to_string() => Price(10.0), "t1".to_string() => Price(11.0) },
        };

        let prepared = SettlementContext {
            tokens,
            limit_orders: orders,
            amm_orders: amms,
        };
        let settlement = convert_settlement(settled, prepared).unwrap();
        assert_eq!(
            settlement.clearing_prices(),
            &hashmap! { t0 => 10.into(), t1 => 11.into() }
        );

        assert_eq!(limit_handler.calls(), vec![7.into()]);
        assert_eq!(
            amm_handler.calls(),
            vec![AmmOrderExecution {
                input: (t0, 8.into()),
                output: (t1, 9.into()),
            }]
        );
    }
}
