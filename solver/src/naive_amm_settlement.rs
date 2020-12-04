use crate::settlement::{Interaction, Settlement, Trade};
use model::{OrderCreation, OrderKind};
use primitive_types::U256;

// Assume both orders are fill-or-kill sell orders and that sell/buy tokens match.
// Result is None if the orders are unmatchable, a direct match if that is possible, or a match
// with a single amm interaction.
pub fn settle_two_fillkill_sell_orders(
    sell_a: &OrderCreation,
    sell_b: &OrderCreation,
) -> Option<Settlement> {
    assert!(&[sell_a, sell_b]
        .iter()
        .all(|order| matches!(order.order_kind, OrderKind::Sell) && !order.partially_fillable));
    assert!(sell_a.sell_token == sell_b.buy_token && sell_b.sell_token == sell_a.buy_token);
    // In a direct match the price and amount requirements and trivially fulfilled.
    let is_direct_match =
        sell_a.sell_amount >= sell_b.buy_amount && sell_b.sell_amount >= sell_a.buy_amount;
    if is_direct_match {
        return Some(direct_match(sell_a, sell_b));
    }

    // For an amm match the amounts don't need to match perfectly but there has to be an overlap
    // in the acceptable prices. The smart contract calculates the executed buy amount as:
    // executed_sell_amount * buy_token_price / sell_token_price
    // From which we get this inequality:
    // sell_b.buy_amount / sell_b.sell_amount <= price_a / price_b <= sell_a.sell_amount / sell_a.buy_amount
    // where [price_a, price_b] is the price vector. These prices can only exist if:
    // sell_a.buy_amount * sell_b.buy_amount <= sell_a.sell_amount * sell_b.sell_amount
    // This transformation avoids lossy division.
    let left = sell_a.buy_amount.checked_mul(sell_b.buy_amount)?;
    let right = sell_a.sell_amount.checked_mul(sell_b.sell_amount)?;
    let have_price_overlap = left <= right;
    if !have_price_overlap {
        return None;
    }
    amm_match_(sell_a, sell_b)
}

// Match two orders directly with their full sell amounts.
fn direct_match(sell_a: &OrderCreation, sell_b: &OrderCreation) -> Settlement {
    Settlement {
        clearing_prices: maplit::hashmap! {
            sell_a.sell_token => sell_a.sell_amount,
            sell_b.sell_token => sell_b.sell_amount,
        },
        trades: vec![
            Trade {
                order: *sell_a,
                executed_amount: sell_a.sell_amount,
            },
            Trade {
                order: *sell_b,
                executed_amount: sell_b.sell_amount,
            },
        ],
        ..Default::default()
    }
}

// Match two orders with amm assuming that there is price overlap.
fn amm_match_(sell_a: &OrderCreation, sell_b: &OrderCreation) -> Option<Settlement> {
    // Based on our assumptions we know that exactly one order is "bigger" than the other in the
    // sense that it a larger sell amount than the other order's buy amount.
    // It is not possible for both orders to be bigger because that would be a direct match which
    // has already been handled.
    let (big, small) = if sell_a.sell_amount > sell_b.buy_amount {
        (sell_a, sell_b)
    } else {
        (sell_b, sell_a)
    };
    // Unwrap because of the above explanation.
    let big_missing_buy_amount = big.buy_amount.checked_sub(small.sell_amount).unwrap();
    // Because the smart contract enforces uniform prices we must pick one price that is accepted by
    // both orders. We know that there is price overlap so we could pick any price between the price
    // limits of the orders. We pick the price of the bigger order because it's sell token surplus
    // will have to be traded with the amm and in this trade it should suffice to receive as few
    // tokens as possible (thus using least constraining price).
    // Because we picked the bigger order's price we must add an extra buy amount to the smaller
    // order so that it gets the same price.
    let small_buy_amount = rounded_up_division(
        small.sell_amount.checked_mul(big.sell_amount)?,
        big.buy_amount,
    )?;
    // Does this have to be saturating or is guaranteed to not underflow? With the rounding from the
    // previous operation I suspect it might.
    let big_extra_sell_amount = big.sell_amount.saturating_sub(small_buy_amount);
    if big_extra_sell_amount.is_zero() {
        return None;
    }

    Some(Settlement {
        clearing_prices: maplit::hashmap! {
            big.sell_token => big.sell_amount,
            big.buy_token => big.buy_amount,
        },
        trades: vec![
            Trade {
                order: *sell_a,
                executed_amount: sell_a.sell_amount,
            },
            Trade {
                order: *sell_b,
                executed_amount: sell_b.sell_amount,
            },
        ],
        interactions: vec![Interaction::AmmSwapExactTokensForTokens {
            amount_in: big_extra_sell_amount,
            amount_out_min: big_missing_buy_amount,
            token_in: big.sell_token,
            token_out: small.sell_token,
        }],
        ..Default::default()
    })
}

fn rounded_up_division(dividend: U256, divisor: U256) -> Option<U256> {
    dividend
        .checked_add(divisor.checked_sub(1.into())?)?
        .checked_div(divisor)
}

#[cfg(test)]
mod tests {
    use super::*;
    use model::OrderKind;
    use primitive_types::H160;

    #[test]
    fn rounded_up_division_() {
        assert_eq!(rounded_up_division(4.into(), 2.into()), Some(2.into()));
        assert_eq!(rounded_up_division(5.into(), 2.into()), Some(3.into()));
        assert_eq!(rounded_up_division(6.into(), 2.into()), Some(3.into()));
        assert_eq!(rounded_up_division(7.into(), 2.into()), Some(4.into()));
        assert_eq!(rounded_up_division(7.into(), 1.into()), Some(7.into()));
        assert_eq!(rounded_up_division(7.into(), 0.into()), None);
    }

    #[test]
    fn direct_match_exact() {
        let sell_a = OrderCreation {
            sell_token: H160::from_low_u64_be(0),
            buy_token: H160::from_low_u64_be(1),
            sell_amount: 5.into(),
            buy_amount: 10.into(),
            order_kind: OrderKind::Sell,
            partially_fillable: false,
            ..Default::default()
        };
        let sell_b = OrderCreation {
            sell_token: H160::from_low_u64_be(1),
            buy_token: H160::from_low_u64_be(0),
            sell_amount: 10.into(),
            buy_amount: 5.into(),
            order_kind: OrderKind::Sell,
            partially_fillable: false,
            ..Default::default()
        };
        // shouldn't matter which order comes first
        for order_pair in [(&sell_a, &sell_b), (&sell_b, &sell_a)].iter().copied() {
            let settlement = settle_two_fillkill_sell_orders(order_pair.0, order_pair.1).unwrap();
            assert_eq!(settlement.clearing_prices.len(), 2);
            assert_eq!(settlement.trades.len(), 2);
            assert!(settlement.interactions.is_empty());
            let clearing_price_a = *settlement.clearing_prices.get(&sell_a.sell_token).unwrap();
            let clearing_price_b = *settlement.clearing_prices.get(&sell_b.sell_token).unwrap();
            assert_eq!(clearing_price_a * 2, clearing_price_b);
        }
    }

    #[test]
    fn direct_match_different_prices() {
        let sell_a = OrderCreation {
            sell_token: H160::from_low_u64_be(0),
            buy_token: H160::from_low_u64_be(1),
            sell_amount: 10.into(),
            buy_amount: 10.into(),
            order_kind: OrderKind::Sell,
            partially_fillable: false,
            ..Default::default()
        };
        let sell_b = OrderCreation {
            sell_token: H160::from_low_u64_be(1),
            buy_token: H160::from_low_u64_be(0),
            sell_amount: 15.into(),
            buy_amount: 5.into(),
            order_kind: OrderKind::Sell,
            partially_fillable: false,
            ..Default::default()
        };
        for order_pair in [(&sell_a, &sell_b), (&sell_b, &sell_a)].iter().copied() {
            let settlement = settle_two_fillkill_sell_orders(order_pair.0, order_pair.1).unwrap();
            assert_eq!(settlement.clearing_prices.len(), 2);
            assert_eq!(settlement.trades.len(), 2);
            assert!(settlement.interactions.is_empty());
            let clearing_price_a = *settlement.clearing_prices.get(&sell_a.sell_token).unwrap();
            let clearing_price_b = *settlement.clearing_prices.get(&sell_b.sell_token).unwrap();
            assert_eq!(clearing_price_a * 3 / 2, clearing_price_b);
        }
    }

    #[test]
    fn unmatchable_because_price() {
        // price of token a in b is at least 1
        let sell_a = OrderCreation {
            sell_token: H160::from_low_u64_be(0),
            buy_token: H160::from_low_u64_be(1),
            sell_amount: 10.into(),
            buy_amount: 10.into(),
            order_kind: OrderKind::Sell,
            partially_fillable: false,
            ..Default::default()
        };
        // price of token a in b is at most 0.5
        let sell_b = OrderCreation {
            sell_token: H160::from_low_u64_be(1),
            buy_token: H160::from_low_u64_be(0),
            sell_amount: 1.into(),
            buy_amount: 2.into(),
            order_kind: OrderKind::Sell,
            partially_fillable: false,
            ..Default::default()
        };
        assert!(settle_two_fillkill_sell_orders(&sell_a, &sell_b).is_none());
        assert!(settle_two_fillkill_sell_orders(&sell_b, &sell_a).is_none());
    }

    #[test]
    fn amm_match_same_price() {
        let sell_a = OrderCreation {
            sell_token: H160::from_low_u64_be(0),
            buy_token: H160::from_low_u64_be(1),
            sell_amount: 10.into(),
            buy_amount: 15.into(),
            order_kind: OrderKind::Sell,
            partially_fillable: false,
            ..Default::default()
        };
        let sell_b = OrderCreation {
            sell_token: H160::from_low_u64_be(1),
            buy_token: H160::from_low_u64_be(0),
            sell_amount: 6.into(),
            buy_amount: 4.into(),
            order_kind: OrderKind::Sell,
            partially_fillable: false,
            ..Default::default()
        };

        let expected_interactions = vec![Interaction::AmmSwapExactTokensForTokens {
            amount_in: 6.into(),
            amount_out_min: 9.into(),
            token_in: sell_a.sell_token,
            token_out: sell_a.buy_token,
        }];

        for order_pair in [(&sell_a, &sell_b), (&sell_b, &sell_a)].iter().copied() {
            let settlement = settle_two_fillkill_sell_orders(order_pair.0, order_pair.1).unwrap();
            assert_eq!(settlement.clearing_prices.len(), 2);
            assert_eq!(settlement.trades.len(), 2);
            assert_eq!(settlement.interactions, expected_interactions);
            let clearing_price_a = *settlement.clearing_prices.get(&sell_a.sell_token).unwrap();
            let clearing_price_b = *settlement.clearing_prices.get(&sell_b.sell_token).unwrap();
            assert_eq!(clearing_price_a * 3 / 2, clearing_price_b);
        }
    }

    #[test]
    fn amm_match_different_price_picks_price_of_bigger_order() {
        // price of token a in b is at least 2
        let sell_a = OrderCreation {
            sell_token: H160::from_low_u64_be(0),
            buy_token: H160::from_low_u64_be(1),
            sell_amount: 10.into(),
            buy_amount: 20.into(),
            order_kind: OrderKind::Sell,
            partially_fillable: false,
            ..Default::default()
        };
        // price of token a in b is at most 4
        let sell_b = OrderCreation {
            sell_token: H160::from_low_u64_be(1),
            buy_token: H160::from_low_u64_be(0),
            sell_amount: 4.into(),
            buy_amount: 1.into(),
            order_kind: OrderKind::Sell,
            partially_fillable: false,
            ..Default::default()
        };

        let expected_interactions = vec![Interaction::AmmSwapExactTokensForTokens {
            amount_in: 8.into(),
            amount_out_min: 16.into(),
            token_in: sell_a.sell_token,
            token_out: sell_a.buy_token,
        }];

        for order_pair in [(&sell_a, &sell_b), (&sell_b, &sell_a)].iter().copied() {
            let settlement = settle_two_fillkill_sell_orders(order_pair.0, order_pair.1).unwrap();
            assert_eq!(settlement.clearing_prices.len(), 2);
            assert_eq!(settlement.trades.len(), 2);
            assert_eq!(settlement.interactions, expected_interactions);
            let clearing_price_a = *settlement.clearing_prices.get(&sell_a.sell_token).unwrap();
            let clearing_price_b = *settlement.clearing_prices.get(&sell_b.sell_token).unwrap();
            assert_eq!(clearing_price_a * 2, clearing_price_b);
        }
    }
}
