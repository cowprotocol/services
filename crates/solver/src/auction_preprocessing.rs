//! Submodule containing helper methods to pre-process auction data before passing it on to the solvers.

use std::collections::{HashMap, HashSet};

use ethcontract::{H160, U256};
use model::order::BUY_ETH_ADDRESS;
use num::BigRational;
use shared::{
    conversions::U256Ext,
    price_estimation::{self, PriceEstimating},
};

use crate::liquidity::LimitOrder;

pub async fn collect_estimated_prices(
    price_estimator: &dyn PriceEstimating,
    native_token_amount_to_estimate_prices_with: U256,
    native_token: H160,
    orders: &[LimitOrder],
) -> HashMap<H160, BigRational> {
    // Computes set of traded tokens (limit orders only).
    // NOTE: The native token is always added.

    let queries = orders
        .iter()
        .flat_map(|order| [order.sell_token, order.buy_token])
        .filter(|token| *token != native_token)
        .collect::<HashSet<_>>()
        .into_iter()
        .map(|token| price_estimation::Query {
            // For ranking purposes it doesn't matter how the external price vector is scaled,
            // but native_token is used here anyway for better logging/debugging.
            sell_token: native_token,
            buy_token: token,
            in_amount: native_token_amount_to_estimate_prices_with,
            kind: model::order::OrderKind::Sell,
        })
        .collect::<Vec<_>>();
    let estimates = price_estimator.estimates(&queries).await;

    fn log_err(token: H160, err: &str) {
        tracing::warn!("failed to estimate price for token {}: {}", token, err);
    }
    let mut prices: HashMap<_, _> = queries
        .into_iter()
        .zip(estimates)
        .filter_map(|(query, estimate)| {
            let estimate = match estimate {
                Ok(estimate) => estimate,
                Err(err) => {
                    log_err(query.buy_token, &format!("{:?}", err));
                    return None;
                }
            };
            let price = match estimate.price_in_sell_token_rational(&query) {
                Some(price) => price,
                None => {
                    log_err(query.buy_token, "infinite price");
                    return None;
                }
            };
            Some((query.buy_token, price))
        })
        .collect();

    // Always include the native token.
    prices.insert(native_token, num::one());
    // And the placeholder for its native counterpart.
    prices.insert(BUY_ETH_ADDRESS, num::one());

    // Derive exchange rate from orders if only one token pair is traded
    augment_prices(orders, &mut prices);

    prices
}

// Filter limit orders for which we don't have price estimates as they cannot be considered for the objective criterion
pub fn orders_with_price_estimates(
    orders: Vec<LimitOrder>,
    prices: &HashMap<H160, BigRational>,
) -> Vec<LimitOrder> {
    let (orders, removed_orders): (Vec<_>, Vec<_>) = orders.into_iter().partition(|order| {
        prices.contains_key(&order.sell_token) && prices.contains_key(&order.buy_token)
    });
    if !removed_orders.is_empty() {
        tracing::debug!(
            "pruned {} orders: {:?}",
            removed_orders.len(),
            removed_orders,
        );
    }
    orders
}

// In case we cannot estimate the price for a token but there is only one pair traded in the batch the accuracy of the price estimate for ranking a solution doesn't matter that much (surplus is not competing with other tokens).
// Therefore we can deduct a price estimate by looking at the average limit price of the orders on the single token pair
fn augment_prices(orders: &[LimitOrder], prices: &mut HashMap<H160, BigRational>) {
    let first_order = match orders.first() {
        Some(order) => order,
        None => return,
    };
    let init = (
        (first_order.sell_token, U256::zero()),
        (first_order.buy_token, U256::zero()),
    );
    let only_pair_with_amounts = orders.iter().try_fold(
        init,
        |((first_token, first_amount), (second_token, second_amount)), order| {
            if first_token == order.sell_token && second_token == order.buy_token {
                // Same pair as previous (same direction)
                Some((
                    (order.sell_token, first_amount + order.sell_amount),
                    (order.buy_token, second_amount + order.buy_amount),
                ))
            } else if first_token == order.buy_token && second_token == order.sell_token {
                // Same pair as previous (opposite direction)
                Some((
                    (order.buy_token, first_amount + order.buy_amount),
                    (order.sell_token, second_amount + order.sell_amount),
                ))
            } else {
                // Don't augment prices unless all orders are on the same pair
                None
            }
        },
    );

    if let Some(((first_token, first_amount), (second_token, second_amount))) =
        only_pair_with_amounts
    {
        if prices.contains_key(&first_token) && !prices.contains_key(&second_token) {
            tracing::debug!("Derived price for {} from limit price", second_token);
            prices.insert(
                second_token,
                prices.get(&first_token).unwrap() * first_amount.to_big_int()
                    / second_amount.to_big_int(),
            );
        }
        if prices.contains_key(&second_token) && !prices.contains_key(&first_token) {
            tracing::debug!("Derived price for {} from limit price", first_token);
            prices.insert(
                first_token,
                prices.get(&second_token).unwrap() * second_amount.to_big_int()
                    / first_amount.to_big_int(),
            );
        }
    }
}

// vk: I would like to extend this to also check that the order has minimum age but for this we need
// access to the creation date which is a more involved change.
pub fn has_at_least_one_user_order(orders: &[LimitOrder]) -> bool {
    orders.iter().any(|order| !order.is_liquidity_order)
}

#[cfg(test)]
mod tests {
    use maplit::hashmap;
    use model::order::OrderKind;
    use num::traits::One as _;
    use shared::price_estimation::mocks::{FailingPriceEstimator, FakePriceEstimator};

    use crate::liquidity::tests::CapturingSettlementHandler;

    use super::*;

    #[tokio::test]
    async fn collect_estimated_prices_adds_prices_for_buy_and_sell_token_of_limit_orders() {
        let price_estimator = FakePriceEstimator(price_estimation::Estimate {
            out_amount: 1.into(),
            gas: 1.into(),
        });

        let native_token = H160::zero();
        let sell_token = H160::from_low_u64_be(1);
        let buy_token = H160::from_low_u64_be(2);

        let orders = vec![LimitOrder {
            sell_amount: 100_000.into(),
            buy_amount: 100_000.into(),
            sell_token,
            buy_token,
            kind: OrderKind::Buy,
            partially_fillable: false,
            scaled_fee_amount: Default::default(),
            settlement_handling: CapturingSettlementHandler::arc(),
            id: "0".into(),
            is_liquidity_order: false,
        }];
        let prices =
            collect_estimated_prices(&price_estimator, 1.into(), native_token, &orders).await;
        assert_eq!(prices.len(), 4);
        assert!(prices.contains_key(&sell_token));
        assert!(prices.contains_key(&buy_token));
    }

    #[tokio::test]
    async fn collect_estimated_prices_skips_token_for_which_estimate_fails() {
        let price_estimator = FailingPriceEstimator();

        let native_token = H160::zero();
        let sell_token = H160::from_low_u64_be(1);
        let buy_token = H160::from_low_u64_be(2);

        let orders = vec![LimitOrder {
            sell_amount: 100_000.into(),
            buy_amount: 100_000.into(),
            sell_token,
            buy_token,
            kind: OrderKind::Buy,
            partially_fillable: false,
            scaled_fee_amount: Default::default(),
            settlement_handling: CapturingSettlementHandler::arc(),
            id: "0".into(),
            is_liquidity_order: false,
        }];
        let prices =
            collect_estimated_prices(&price_estimator, 1.into(), native_token, &orders).await;
        assert_eq!(prices.len(), 2);
    }

    #[tokio::test]
    async fn collect_estimated_prices_adds_native_token_if_wrapped_is_traded() {
        let price_estimator = FakePriceEstimator(price_estimation::Estimate {
            out_amount: 1.into(),
            gas: 1.into(),
        });

        let native_token = H160::zero();
        let sell_token = H160::from_low_u64_be(1);

        let liquidity = vec![LimitOrder {
            sell_amount: 100_000.into(),
            buy_amount: 100_000.into(),
            sell_token,
            buy_token: native_token,
            kind: OrderKind::Buy,
            partially_fillable: false,
            scaled_fee_amount: Default::default(),
            settlement_handling: CapturingSettlementHandler::arc(),
            id: "0".into(),
            is_liquidity_order: false,
        }];
        let prices =
            collect_estimated_prices(&price_estimator, 1.into(), native_token, &liquidity).await;
        assert_eq!(prices.len(), 3);
        assert!(prices.contains_key(&sell_token));
        assert!(prices.contains_key(&native_token));
        assert!(prices.contains_key(&BUY_ETH_ADDRESS));
    }

    #[test]
    fn liquidity_with_price_removes_liquidity_without_price() {
        let tokens = [
            H160::from_low_u64_be(0),
            H160::from_low_u64_be(1),
            H160::from_low_u64_be(2),
            H160::from_low_u64_be(3),
        ];
        let prices = hashmap! {tokens[0] => BigRational::one(), tokens[1] => BigRational::one()};
        let order = |sell_token, buy_token| LimitOrder {
            sell_token,
            buy_token,
            ..Default::default()
        };
        let orders = vec![
            order(tokens[0], tokens[1]),
            order(tokens[0], tokens[2]),
            order(tokens[2], tokens[0]),
            order(tokens[2], tokens[3]),
        ];
        let filtered = orders_with_price_estimates(orders, &prices);
        assert_eq!(filtered.len(), 1);
        assert!(filtered[0].sell_token == tokens[0] && filtered[0].buy_token == tokens[1]);
    }

    #[test]
    fn test_augment_prices() {
        let eth = H160::from_low_u64_be(0);
        let gno = H160::from_low_u64_be(1);
        let mut prices = hashmap! {
            eth => BigRational::one()
        };
        let mut orders = vec![
            LimitOrder {
                sell_token: gno,
                sell_amount: 10.into(),
                buy_token: eth,
                buy_amount: 1.into(),
                ..Default::default()
            },
            LimitOrder {
                sell_token: gno,
                sell_amount: 8.into(),
                buy_token: eth,
                buy_amount: 1.into(),
                ..Default::default()
            },
            LimitOrder {
                sell_token: eth,
                sell_amount: 2.into(),
                buy_token: gno,
                buy_amount: 18.into(),
                ..Default::default()
            },
        ];

        let mut cloned_prices = prices.clone();
        augment_prices(&orders, &mut cloned_prices);
        assert_eq!(
            cloned_prices.get(&gno).unwrap(),
            &BigRational::new(4.into(), 36.into())
        );

        // Having ETH->GNO order first doesn't change anything
        orders.reverse();
        augment_prices(&orders, &mut prices);
        assert_eq!(
            prices.get(&gno).unwrap(),
            &BigRational::new(4.into(), 36.into())
        );
    }

    #[test]
    fn test_does_not_augment_prices_if_more_than_one_pair() {
        let eth = H160::from_low_u64_be(0);
        let gno = H160::from_low_u64_be(1);
        let dai = H160::from_low_u64_be(2);
        let mut prices = hashmap! {
            eth => BigRational::one()
        };
        let orders = vec![
            LimitOrder {
                sell_token: gno,
                sell_amount: 10.into(),
                buy_token: eth,
                buy_amount: 1.into(),
                ..Default::default()
            },
            LimitOrder {
                sell_token: gno,
                sell_amount: 1.into(),
                buy_token: dai,
                buy_amount: 400.into(),
                ..Default::default()
            },
            LimitOrder {
                sell_token: gno,
                sell_amount: 10.into(),
                buy_token: eth,
                buy_amount: 1.into(),
                ..Default::default()
            },
        ];

        augment_prices(&orders, &mut prices);
        assert_eq!(prices.len(), 1);
    }
}
