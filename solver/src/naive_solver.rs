mod two_order_settlement;

use crate::settlement::Settlement;
use model::{OrderCreation, OrderKind, TokenPair};
use primitive_types::U512;
use std::{cmp::Ordering, collections::HashMap};

pub fn settle(orders: impl Iterator<Item = OrderCreation>) -> Option<Settlement> {
    let orders = organize_orders_by_token_pair(orders);
    // TODO: Settle multiple token pairs in one settlement.
    orders
        .into_iter()
        .find_map(|(_, orders)| settle_pair(orders))
}

fn settle_pair(orders: TokenPairOrders) -> Option<Settlement> {
    let most_lenient_a = orders.sell_token_0.into_iter().min_by(order_by_price)?;
    let most_lenient_b = orders.sell_token_1.into_iter().min_by(order_by_price)?;
    two_order_settlement::settle_two_fillkill_sell_orders(&most_lenient_a, &most_lenient_b)
}

#[derive(Debug, Default)]
struct TokenPairOrders {
    sell_token_0: Vec<OrderCreation>,
    sell_token_1: Vec<OrderCreation>,
}

fn organize_orders_by_token_pair(
    orders: impl Iterator<Item = OrderCreation>,
) -> HashMap<TokenPair, TokenPairOrders> {
    let mut result = HashMap::<_, TokenPairOrders>::new();
    for (order, token_pair) in orders
        .filter(usable_order)
        .filter_map(|order| Some((order, order.token_pair()?)))
    {
        let token_pair_orders = result.entry(token_pair).or_default();
        if order.sell_token == token_pair.get().0 {
            token_pair_orders.sell_token_0.push(order);
        } else {
            token_pair_orders.sell_token_1.push(order);
        }
    }
    result
}

fn usable_order(order: &OrderCreation) -> bool {
    matches!(order.kind, OrderKind::Sell)
        && !order.sell_amount.is_zero()
        && !order.buy_amount.is_zero()
        && !order.partially_fillable
}

fn order_by_price(a: &OrderCreation, b: &OrderCreation) -> Ordering {
    // The natural ordering is `a.buy_amount / a.sell_amount < b.buy_amount / b.sell_amount`
    // which we can transform to `a.buy_amount * b.sell_amount < b.buy_amount * b.sell_amount` to
    // avoid division. Multiply in u512 to avoid overflow.
    let left = U512::from(a.buy_amount) * U512::from(b.sell_amount);
    let right = U512::from(b.buy_amount) * U512::from(a.sell_amount);
    left.cmp(&right)
}

#[cfg(test)]
mod tests {
    use super::*;
    use primitive_types::{H160, U256};

    fn order_with_amounts(sell_amount: U256, buy_amount: U256) -> OrderCreation {
        OrderCreation {
            sell_amount,
            buy_amount,
            ..Default::default()
        }
    }

    #[test]
    fn order_by_price_() {
        let right = &order_with_amounts(10.into(), 10.into());

        let left = &order_with_amounts(10.into(), 10.into());
        assert_eq!(order_by_price(&left, &right), Ordering::Equal);

        let left = &order_with_amounts(9.into(), 9.into());
        assert_eq!(order_by_price(&left, &right), Ordering::Equal);

        let left = &order_with_amounts(9.into(), 10.into());
        assert_eq!(order_by_price(&left, &right), Ordering::Greater);

        let left = &order_with_amounts(10.into(), 11.into());
        assert_eq!(order_by_price(&left, &right), Ordering::Greater);

        let left = &order_with_amounts(10.into(), 9.into());
        assert_eq!(order_by_price(&left, &right), Ordering::Less);

        let left = &order_with_amounts(11.into(), 10.into());
        assert_eq!(order_by_price(&left, &right), Ordering::Less);
    }

    #[test]
    fn settle_finds_match() {
        let orders = vec![
            OrderCreation {
                sell_token: H160::from_low_u64_be(0),
                buy_token: H160::from_low_u64_be(1),
                sell_amount: 4.into(),
                buy_amount: 9.into(),
                kind: OrderKind::Sell,
                partially_fillable: false,
                ..Default::default()
            },
            OrderCreation {
                sell_token: H160::from_low_u64_be(0),
                buy_token: H160::from_low_u64_be(1),
                sell_amount: 4.into(),
                buy_amount: 8.into(),
                kind: OrderKind::Sell,
                partially_fillable: false,
                ..Default::default()
            },
            OrderCreation {
                sell_token: H160::from_low_u64_be(1),
                buy_token: H160::from_low_u64_be(0),
                sell_amount: 10.into(),
                buy_amount: 11.into(),
                kind: OrderKind::Sell,
                partially_fillable: false,
                ..Default::default()
            },
            OrderCreation {
                sell_token: H160::from_low_u64_be(1),
                buy_token: H160::from_low_u64_be(0),
                sell_amount: 6.into(),
                buy_amount: 2.into(),
                kind: OrderKind::Sell,
                partially_fillable: false,
                ..Default::default()
            },
        ];

        let settlement = settle(orders.into_iter()).unwrap();
        dbg!(&settlement);
        assert_eq!(settlement.trades.len(), 2);
        assert_eq!(settlement.interactions.len(), 1);
    }
}
