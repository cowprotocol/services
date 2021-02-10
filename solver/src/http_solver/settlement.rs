use super::model::{Price, SettledBatchAuctionModel};
use crate::{
    liquidity::{AmmOrder, LimitOrder},
    settlement::Settlement,
};
use anyhow::{anyhow, ensure, Result};
use model::order::OrderKind;
use primitive_types::{H160, U256};
use std::collections::HashMap;

pub fn convert_settlement(
    model: &SettledBatchAuctionModel,
    tokens: &HashMap<String, H160>,
    orders: &HashMap<String, &LimitOrder>,
    amms: &HashMap<String, &AmmOrder>,
) -> Result<Settlement> {
    let mut settlement = Settlement::default();
    set_orders(model, orders, &mut settlement)?;
    set_amms(model, amms, &mut settlement)?;
    set_prices(model, tokens, &mut settlement)?;
    Ok(settlement)
}

fn set_orders(
    model: &SettledBatchAuctionModel,
    orders: &HashMap<String, &LimitOrder>,
    settlement: &mut Settlement,
) -> Result<()> {
    for (index, model) in model.orders.iter() {
        let order = orders
            .get(index.as_str())
            .ok_or_else(|| anyhow!("invalid order {}", index))?;
        let executed_amount = match order.kind {
            OrderKind::Buy => model.exec_buy_amount,
            OrderKind::Sell => model.exec_sell_amount,
        };
        if executed_amount.is_zero() {
            continue;
        }
        let (trade, interactions) = order.settlement_handling.settle(executed_amount);
        if let Some(trade) = trade {
            settlement.trades.push(trade);
        }
        settlement.interactions.extend(interactions);
    }
    Ok(())
}

fn set_amms(
    model: &SettledBatchAuctionModel,
    amms: &HashMap<String, &AmmOrder>,
    settlement: &mut Settlement,
) -> Result<()> {
    for (index, model) in model.uniswaps.iter() {
        let amm = amms
            .get(index.as_str())
            .ok_or_else(|| anyhow!("invalid amm {}", index))?;
        let (input, output) =
            if model.balance_update_1.is_positive() && model.balance_update_2.is_negative() {
                (
                    (amm.tokens.get().0, i128_abs_to_u256(model.balance_update_1)),
                    (amm.tokens.get().1, i128_abs_to_u256(model.balance_update_2)),
                )
            } else if model.balance_update_2.is_positive() && model.balance_update_1.is_negative() {
                (
                    (amm.tokens.get().1, i128_abs_to_u256(model.balance_update_2)),
                    (amm.tokens.get().0, i128_abs_to_u256(model.balance_update_1)),
                )
            } else if model.balance_update_1 == 0 && model.balance_update_2 == 0 {
                continue;
            } else {
                return Err(anyhow!("invalid uniswap update {:?}", model));
            };
        let interactions = amm.settlement_handling.settle(input, output);
        settlement.interactions.extend(interactions);
    }
    Ok(())
}

fn set_prices(
    model: &SettledBatchAuctionModel,
    tokens: &HashMap<String, H160>,
    settlement: &mut Settlement,
) -> Result<()> {
    for (index, &Price(price)) in model.prices.iter() {
        let token = tokens
            .get(index.as_str())
            .ok_or_else(|| anyhow!("invalid token {}", index))?;
        let token_used_in_trade = settlement
            .trades
            .iter()
            .any(|trade| *token == trade.order.sell_token || *token == trade.order.buy_token);
        if token_used_in_trade {
            ensure!(price.is_finite() && price > 0.0, "invalid price {}", price);
            let price = U256::from_f64_lossy(price);
            settlement.clearing_prices.insert(*token, price);
        }
    }
    Ok(())
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
        http_solver::model::{ExecutedOrderModel, UpdatedUniswapModel},
        liquidity::{MockAmmSettlementHandling, MockLimitOrderSettlementHandling},
        settlement::{Interaction, Trade},
    };
    use maplit::hashmap;
    use mockall::predicate::eq;
    use model::{order::OrderCreation, TokenPair};
    use std::sync::Arc;

    #[derive(Debug)]
    struct NoopInteraction;
    impl Interaction for NoopInteraction {
        fn encode(&self, _writer: &mut dyn std::io::Write) -> Result<()> {
            Ok(())
        }
    }

    #[test]
    fn convert_settlement_() {
        let t0 = H160::from_low_u64_be(0);
        let t1 = H160::from_low_u64_be(1);
        let tokens = hashmap! { "t0".to_string() => t0, "t1".to_string() => t1 };

        let mut limit_handling = MockLimitOrderSettlementHandling::new();
        limit_handling.expect_settle().returning(move |_| {
            (
                Some(Trade {
                    order: OrderCreation {
                        sell_token: t0,
                        buy_token: t1,
                        sell_amount: 1.into(),
                        buy_amount: 2.into(),
                        ..Default::default()
                    },
                    ..Default::default()
                }),
                vec![Box::new(NoopInteraction)],
            )
        });
        let limit_order = LimitOrder {
            sell_token: t0,
            buy_token: t1,
            sell_amount: 1.into(),
            buy_amount: 2.into(),
            kind: OrderKind::Sell,
            partially_fillable: false,
            settlement_handling: Arc::new(limit_handling),
        };
        let orders = hashmap! { "lo0".to_string() => &limit_order };

        let mut amm_handling = MockAmmSettlementHandling::new();
        amm_handling
            .expect_settle()
            .with(eq((t0, 8.into())), eq((t1, 9.into())))
            .returning(|_, _| vec![Box::new(NoopInteraction)]);
        let amm_order = AmmOrder {
            tokens: TokenPair::new(t0, t1).unwrap(),
            reserves: (3, 4),
            fee: 5.into(),
            settlement_handling: Arc::new(amm_handling),
        };
        let amms = hashmap! { "amm0".to_string() => &amm_order };

        let executed_order = ExecutedOrderModel {
            exec_buy_amount: 6.into(),
            exec_sell_amount: 7.into(),
        };
        let updated_uniswap = UpdatedUniswapModel {
            balance_update_1: 8,
            balance_update_2: -9,
        };
        let model = SettledBatchAuctionModel {
            orders: hashmap! { "lo0".to_string() => executed_order },
            uniswaps: hashmap! { "amm0".to_string() => updated_uniswap },
            ref_token: "t0".to_string(),
            prices: hashmap! { "t0".to_string() => Price(10.0), "t1".to_string() => Price(11.0) },
        };

        let settlement = convert_settlement(&model, &tokens, &orders, &amms).unwrap();
        assert_eq!(
            settlement.clearing_prices,
            hashmap! { t0 => 10.into(), t1 => 11.into() }
        );
        assert_eq!(settlement.trades.len(), 1);
        assert_eq!(settlement.interactions.len(), 2);
    }
}
