use crate::{
    liquidity,
    settlement::{Interaction, Settlement, Trade},
};
use anyhow::{anyhow, Result};
use liquidity::{AmmOrder, LimitOrder};
use model::order::OrderKind;
use num::{bigint::Sign, BigInt};
use std::collections::HashMap;
use web3::types::{Address, U256};

#[derive(Debug)]
struct TokenContext {
    address: Address,
    reserve: U256,
    buy_volume: U256,
    sell_volume: U256,
}

// TODO use concrete fee from AMMOrder
impl TokenContext {
    pub fn is_excess_after_fees(&self, deficit: &TokenContext) -> bool {
        1000 * u256_to_bigint(&self.reserve)
            * (u256_to_bigint(&deficit.sell_volume) - u256_to_bigint(&deficit.buy_volume))
            < 997
                * u256_to_bigint(&deficit.reserve)
                * (u256_to_bigint(&self.sell_volume) - u256_to_bigint(&self.buy_volume))
    }

    pub fn is_excess_before_fees(&self, deficit: &TokenContext) -> bool {
        u256_to_bigint(&self.reserve)
            * (u256_to_bigint(&deficit.sell_volume) - u256_to_bigint(&deficit.buy_volume))
            < u256_to_bigint(&deficit.reserve)
                * (u256_to_bigint(&self.sell_volume) - u256_to_bigint(&self.buy_volume))
    }
}

pub fn solve(orders: impl Iterator<Item = LimitOrder> + Clone, pool: &AmmOrder) -> Settlement {
    let mut orders: Vec<LimitOrder> = orders.collect();
    while !orders.is_empty() {
        let (context_a, context_b) = split_into_contexts(orders.clone().into_iter(), pool);
        let solution = solve_orders(orders.clone().into_iter(), &pool, &context_a, &context_b);
        if is_valid_solution(&solution) {
            return solution;
        } else {
            // remove order with worst limit price that is selling excess token (to make it less excessive) and try again
            let excess_token = if context_a.is_excess_before_fees(&context_b) {
                context_a.address
            } else {
                context_b.address
            };
            let order_to_remove = orders
                .iter()
                .enumerate()
                .filter(|o| o.1.sell_token == excess_token)
                .max_by(|lhs, rhs| {
                    (lhs.1.buy_amount * rhs.1.sell_amount)
                        .cmp(&(lhs.1.sell_amount * rhs.1.buy_amount))
                });
            match order_to_remove {
                Some((index, _)) => orders.swap_remove(index),
                None => break,
            };
        }
    }

    // At last we return the trivial solution which doesn't match any orders
    Settlement {
        clearing_prices: HashMap::new(),
        trades: Vec::new(),
        interactions: Vec::new(),
        ..Default::default()
    }
}

///
/// Computes a settlement using orders of a single pair and the direct AMM between those tokens.get(.
/// Panics if orders are not already filtered for a specific token pair, or the reserve information
/// for that pair is not available.
///
fn solve_orders(
    orders: impl Iterator<Item = LimitOrder> + Clone,
    pool: &AmmOrder,
    context_a: &TokenContext,
    context_b: &TokenContext,
) -> Settlement {
    if context_a.is_excess_after_fees(&context_b) {
        solve_with_uniswap(orders, pool, &context_b, &context_a)
    } else if context_b.is_excess_after_fees(&context_a) {
        solve_with_uniswap(orders, pool, &context_a, &context_b)
    } else {
        solve_without_uniswap(orders, &context_a, &context_b)
    }
}

///
/// Creates a solution using the current AMM spot price, without using any of its liquidity
///
fn solve_without_uniswap(
    orders: impl Iterator<Item = LimitOrder> + Clone,
    context_a: &TokenContext,
    context_b: &TokenContext,
) -> Settlement {
    let (trades, interactions) = fully_matched(orders);
    Settlement {
        clearing_prices: maplit::hashmap! {
            context_a.address => context_b.reserve,
            context_b.address => context_a.reserve,
        },
        trades,
        interactions,
        ..Default::default()
    }
}

///
/// Creates a solution using the current AMM's liquidity to balance excess and shortage.
/// The clearing price is the effective exchange rate used by the AMM interaction.
///
fn solve_with_uniswap(
    orders: impl Iterator<Item = LimitOrder> + Clone,
    pool: &AmmOrder,
    shortage: &TokenContext,
    excess: &TokenContext,
) -> Settlement {
    let uniswap_out = compute_uniswap_out(&shortage, &excess);
    let uniswap_in = compute_uniswap_in(uniswap_out, &shortage, &excess);

    let (trades, mut interactions) = fully_matched(orders);
    interactions.extend(pool.settlement_handling.settle(
        (excess.address, uniswap_in),
        (shortage.address, uniswap_out),
    ));
    Settlement {
        clearing_prices: maplit::hashmap! {
            shortage.address => uniswap_in,
            excess.address => uniswap_out,
        },
        trades,
        interactions,
        ..Default::default()
    }
}

fn fully_matched(
    orders: impl Iterator<Item = LimitOrder> + Clone,
) -> (Vec<Trade>, Vec<Box<dyn Interaction>>) {
    let mut trades = Vec::new();
    let mut interactions = Vec::new();
    for order in orders {
        let executed_amount = match order.kind {
            model::order::OrderKind::Buy => order.buy_amount,
            model::order::OrderKind::Sell => order.sell_amount,
        };
        let (trade, trade_interactions) = order.settlement_handling.settle(executed_amount);
        if let Some(trade) = trade {
            trades.push(trade)
        }
        interactions.extend(trade_interactions);
    }
    (trades, interactions)
}

impl AmmOrder {
    fn get_reserve(&self, token: &Address) -> Option<U256> {
        if &self.tokens.get().0 == token {
            Some(self.reserves.0.into())
        } else if &self.tokens.get().1 == token {
            Some(self.reserves.1.into())
        } else {
            None
        }
    }
}

fn split_into_contexts(
    orders: impl Iterator<Item = LimitOrder>,
    pool: &AmmOrder,
) -> (TokenContext, TokenContext) {
    let mut contexts = HashMap::new();
    for order in orders {
        let buy_context = contexts
            .entry(order.buy_token)
            .or_insert_with(|| TokenContext {
                address: order.buy_token,
                reserve: pool
                    .get_reserve(&order.buy_token)
                    .unwrap_or_else(|| panic!("No reserve for token {}", &order.buy_token)),
                buy_volume: U256::zero(),
                sell_volume: U256::zero(),
            });
        if matches!(order.kind, OrderKind::Buy) {
            buy_context.buy_volume += order.buy_amount
        }

        let sell_context = contexts
            .entry(order.sell_token)
            .or_insert_with(|| TokenContext {
                address: order.sell_token,
                reserve: pool
                    .get_reserve(&order.sell_token)
                    .unwrap_or_else(|| panic!("No reserve for token {}", &order.sell_token)),
                buy_volume: U256::zero(),
                sell_volume: U256::zero(),
            });
        if matches!(order.kind, OrderKind::Sell) {
            sell_context.sell_volume += order.sell_amount
        }
    }
    assert!(contexts.len() == 2, "Orders contain more than two tokens");
    let mut contexts = contexts.drain().map(|(_, v)| v);
    (contexts.next().unwrap(), contexts.next().unwrap())
}

///
/// Given information about the shortage token (the one we need to take from Uniswap) and the excess token (the one we give to Uniswap), this function
/// computes the exact out_amount required from Uniswap to perfectly match demand and supply at the effective Uniswap price (the one used for that in/out swap).
///
/// The derivation of this formula is described in https://docs.google.com/document/d/1jS22wxbCqo88fGsqEMZgRQgiAcHlPqxoMw3CJTHst6c/edit
/// It assumes GP fee (φ) to be 1 and Uniswap fee (Φ) to be 0.997
///
fn compute_uniswap_out(shortage: &TokenContext, excess: &TokenContext) -> U256 {
    let numerator_minuend = 997
        * (u256_to_bigint(&excess.sell_volume) - u256_to_bigint(&excess.buy_volume))
        * u256_to_bigint(&shortage.reserve);
    let numerator_subtrahend = 1000
        * (u256_to_bigint(&shortage.sell_volume) - u256_to_bigint(&shortage.buy_volume))
        * u256_to_bigint(&excess.reserve);
    let denominator = (1000 * u256_to_bigint(&excess.reserve))
        + (997 * (u256_to_bigint(&excess.sell_volume) - u256_to_bigint(&excess.buy_volume)));
    bigint_to_u256(&((numerator_minuend - numerator_subtrahend) / denominator))
        .expect("uniswap_out should always be U256 compatible if excess is chosen correctly")
}

///
/// Given the desired amount to receive and the state of the pool, this computes the required amount
/// of tokens to be sent to the pool.
/// Taken from: https://github.com/Uniswap/uniswap-v2-periphery/blob/4123f93278b60bcf617130629c69d4016f9e7584/contracts/libraries/UniswapV2Library.sol#L53
///
fn compute_uniswap_in(out: U256, shortage: &TokenContext, excess: &TokenContext) -> U256 {
    U256::from(1000) * out * excess.reserve / (U256::from(997) * (shortage.reserve - out)) + 1
}

///
/// Returns true if for each trade the executed price is not smaller than the limit price
/// Thus we ensure that `buy_token_price / sell_token_price >= limit_buy_amount / limit_sell_amount`
///
fn is_valid_solution(solution: &Settlement) -> bool {
    for trade in solution.trades.iter() {
        let order = trade.order;
        let buy_token_price = solution
            .clearing_prices
            .get(&order.buy_token)
            .expect("Solution should contain clearing price for buy token");
        let sell_token_price = solution
            .clearing_prices
            .get(&order.sell_token)
            .expect("Solution should contain clearing price for sell token");

        if order.sell_amount * sell_token_price < order.buy_amount * buy_token_price {
            return false;
        }
    }

    true
}

fn u256_to_bigint(input: &U256) -> BigInt {
    let mut bytes = [0; 32];
    input.to_big_endian(&mut bytes);
    BigInt::from_bytes_be(Sign::Plus, &bytes)
}

fn bigint_to_u256(input: &BigInt) -> Result<U256> {
    let (sign, bytes) = input.to_bytes_be();
    if sign == Sign::Minus {
        return Err(anyhow!("Negative BigInt to U256 conversion"));
    }
    if bytes.len() > 32 {
        return Err(anyhow!("BigInt too big for U256 conversion"));
    }
    Ok(U256::from_big_endian(&bytes))
}

#[cfg(test)]
mod tests {
    use liquidity::tests::{CapturingAmmSettlementHandler, CapturingLimitOrderSettlementHandler};
    use model::{order::OrderCreation, TokenPair};
    use num::Rational;

    use super::*;

    fn to_wei(base: u128) -> U256 {
        U256::from(base) * U256::from(10).pow(18.into())
    }

    #[test]
    fn finds_clearing_price_with_sell_orders_on_both_sides() {
        let token_a = Address::from_low_u64_be(0);
        let token_b = Address::from_low_u64_be(1);
        let orders = vec![
            LimitOrder {
                sell_token: token_a,
                buy_token: token_b,
                sell_amount: to_wei(40),
                buy_amount: to_wei(30),
                kind: OrderKind::Sell,
                partially_fillable: false,
                settlement_handling: CapturingLimitOrderSettlementHandler::arc(),
            },
            LimitOrder {
                sell_token: token_b,
                buy_token: token_a,
                sell_amount: to_wei(100),
                buy_amount: to_wei(90),
                kind: OrderKind::Sell,
                partially_fillable: false,
                settlement_handling: CapturingLimitOrderSettlementHandler::arc(),
            },
        ];

        let amm_handler = CapturingAmmSettlementHandler::arc();
        let pool = AmmOrder {
            tokens: TokenPair::new(token_a, token_b).unwrap(),
            reserves: (to_wei(1000).as_u128(), to_wei(1000).as_u128()),
            fee: Rational::new(3, 1000),
            settlement_handling: amm_handler.clone(),
        };
        let result = solve(orders.clone().into_iter(), &pool);

        // Make sure the uniswap interaction is using the correct direction
        let interaction = amm_handler.calls()[0];
        assert_eq!(interaction.token_in, token_b);
        assert_eq!(interaction.token_out, token_a);

        // Make sure the sell amounts +/- uniswap interaction satisfy min_buy amounts
        assert!(orders[0].sell_amount + interaction.amount_out >= orders[1].buy_amount);
        assert!(orders[1].sell_amount - interaction.amount_in > orders[0].buy_amount);

        // Make sure the sell amounts +/- uniswap interaction satisfy expected buy amounts given clearing price
        let price_a = result.clearing_prices.get(&token_a).unwrap();
        let price_b = result.clearing_prices.get(&token_b).unwrap();

        // Multiplying sellAmount with priceA, gives us sell value in "$", divided by priceB gives us value in buy token
        // We should have at least as much to give (sell amount +/- uniswap) as is expected by the buyer
        let expected_buy = orders[0].sell_amount * price_a / price_b;
        assert!(orders[1].sell_amount - interaction.amount_in >= expected_buy);

        let expected_buy = orders[1].sell_amount * price_b / price_a;
        assert!(orders[0].sell_amount + interaction.amount_out >= expected_buy);
    }

    #[test]
    fn finds_clearing_price_with_sell_orders_on_one_side() {
        let token_a = Address::from_low_u64_be(0);
        let token_b = Address::from_low_u64_be(1);
        let orders = vec![
            LimitOrder {
                sell_token: token_a,
                buy_token: token_b,
                sell_amount: to_wei(40),
                buy_amount: to_wei(30),
                kind: OrderKind::Sell,
                partially_fillable: false,
                settlement_handling: CapturingLimitOrderSettlementHandler::arc(),
            },
            LimitOrder {
                sell_token: token_a,
                buy_token: token_b,
                sell_amount: to_wei(100),
                buy_amount: to_wei(90),
                kind: OrderKind::Sell,
                partially_fillable: false,
                settlement_handling: CapturingLimitOrderSettlementHandler::arc(),
            },
        ];

        let amm_handler = CapturingAmmSettlementHandler::arc();
        let pool = AmmOrder {
            tokens: TokenPair::new(token_a, token_b).unwrap(),
            reserves: (to_wei(1_000_000).as_u128(), to_wei(1_000_000).as_u128()),
            fee: Rational::new(3, 1000),
            settlement_handling: amm_handler.clone(),
        };
        let result = solve(orders.clone().into_iter(), &pool);

        // Make sure the uniswap interaction is using the correct direction
        let interaction = amm_handler.calls()[0];
        assert_eq!(interaction.token_in, token_a);
        assert_eq!(interaction.token_out, token_b);

        // Make sure the sell amounts cover the uniswap in, and min buy amounts are covered by uniswap out
        assert!(orders[0].sell_amount + orders[1].sell_amount >= interaction.amount_in);
        assert!(interaction.amount_out >= orders[0].buy_amount + orders[1].buy_amount);

        // Make sure expected buy amounts (given prices) are also covered by uniswap out amounts
        let price_a = result.clearing_prices.get(&token_a).unwrap();
        let price_b = result.clearing_prices.get(&token_b).unwrap();

        let first_expected_buy = orders[0].sell_amount * price_a / price_b;
        let second_expected_buy = orders[1].sell_amount * price_a / price_b;
        assert!(interaction.amount_out >= first_expected_buy + second_expected_buy);
    }

    #[test]
    fn finds_clearing_price_with_buy_orders_on_both_sides() {
        let token_a = Address::from_low_u64_be(0);
        let token_b = Address::from_low_u64_be(1);
        let orders = vec![
            LimitOrder {
                sell_token: token_a,
                buy_token: token_b,
                sell_amount: to_wei(40),
                buy_amount: to_wei(30),
                kind: OrderKind::Buy,
                partially_fillable: false,
                settlement_handling: CapturingLimitOrderSettlementHandler::arc(),
            },
            LimitOrder {
                sell_token: token_b,
                buy_token: token_a,
                sell_amount: to_wei(100),
                buy_amount: to_wei(90),
                kind: OrderKind::Buy,
                partially_fillable: false,
                settlement_handling: CapturingLimitOrderSettlementHandler::arc(),
            },
        ];

        let amm_handler = CapturingAmmSettlementHandler::arc();
        let pool = AmmOrder {
            tokens: TokenPair::new(token_a, token_b).unwrap(),
            reserves: (to_wei(1000).as_u128(), to_wei(1000).as_u128()),
            fee: Rational::new(3, 1000),
            settlement_handling: amm_handler.clone(),
        };
        let result = solve(orders.clone().into_iter(), &pool);

        // Make sure the uniswap interaction is using the correct direction
        let interaction = amm_handler.calls()[0];
        assert_eq!(interaction.token_in, token_b);
        assert_eq!(interaction.token_out, token_a);

        // Make sure the buy amounts +/- uniswap interaction satisfy max_sell amounts
        assert!(orders[0].sell_amount >= orders[1].buy_amount - interaction.amount_out);
        assert!(orders[1].sell_amount >= orders[0].buy_amount + interaction.amount_in);

        // Make sure buy sell amounts +/- uniswap interaction satisfy expected sell amounts given clearing price
        let price_a = result.clearing_prices.get(&token_a).unwrap();
        let price_b = result.clearing_prices.get(&token_b).unwrap();

        // Multiplying buyAmount with priceB, gives us sell value in "$", divided by priceA gives us value in sell token
        // The seller should expect to sell at least as much as we require for the buyer + uniswap.
        let expected_sell = orders[0].buy_amount * price_b / price_a;
        assert!(orders[1].buy_amount - interaction.amount_in <= expected_sell);

        let expected_sell = orders[1].buy_amount * price_a / price_b;
        assert!(orders[0].buy_amount + interaction.amount_out <= expected_sell);
    }

    #[test]
    fn finds_clearing_price_with_buy_orders_and_sell_orders() {
        let token_a = Address::from_low_u64_be(0);
        let token_b = Address::from_low_u64_be(1);
        let orders = vec![
            LimitOrder {
                sell_token: token_a,
                buy_token: token_b,
                sell_amount: to_wei(40),
                buy_amount: to_wei(30),
                kind: OrderKind::Buy,
                partially_fillable: false,
                settlement_handling: CapturingLimitOrderSettlementHandler::arc(),
            },
            LimitOrder {
                sell_token: token_b,
                buy_token: token_a,
                sell_amount: to_wei(100),
                buy_amount: to_wei(90),
                kind: OrderKind::Sell,
                partially_fillable: false,
                settlement_handling: CapturingLimitOrderSettlementHandler::arc(),
            },
        ];

        let amm_handler = CapturingAmmSettlementHandler::arc();
        let pool = AmmOrder {
            tokens: TokenPair::new(token_a, token_b).unwrap(),
            reserves: (to_wei(1000).as_u128(), to_wei(1000).as_u128()),
            fee: Rational::new(3, 1000),
            settlement_handling: amm_handler.clone(),
        };
        let result = solve(orders.clone().into_iter(), &pool);

        // Make sure the uniswap interaction is using the correct direction
        let interaction = amm_handler.calls()[0];
        assert_eq!(interaction.token_in, token_b);
        assert_eq!(interaction.token_out, token_a);

        // Make sure the buy order's sell amount - uniswap interaction satisfies sell order's limit
        assert!(orders[0].sell_amount >= orders[1].buy_amount - interaction.amount_out);

        // Make sure the sell order's buy amount + uniswap interaction satisfies buy order's limit
        assert!(orders[1].buy_amount + interaction.amount_in >= orders[0].sell_amount);

        // Make sure buy sell amounts +/- uniswap interaction satisfy expected sell amounts given clearing price
        let price_a = result.clearing_prices.get(&token_a).unwrap();
        let price_b = result.clearing_prices.get(&token_b).unwrap();

        // Multiplying buy_amount with priceB, gives us sell value in "$", divided by priceA gives us value in sell token
        // The seller should expect to sell at least as much as we require for the buyer + uniswap.
        let expected_sell = orders[0].buy_amount * price_b / price_a;
        assert!(orders[1].buy_amount - interaction.amount_in <= expected_sell);

        // Multiplying sell_amount with priceA, gives us sell value in "$", divided by priceB gives us value in buy token
        // We should have at least as much to give (sell amount + uniswap out) as is expected by the buyer
        let expected_buy = orders[1].sell_amount * price_b / price_a;
        assert!(orders[0].sell_amount + interaction.amount_out >= expected_buy);
    }

    #[test]
    fn finds_clearing_without_using_uniswap() {
        let token_a = Address::from_low_u64_be(0);
        let token_b = Address::from_low_u64_be(1);
        let orders = vec![
            LimitOrder {
                sell_token: token_a,
                buy_token: token_b,
                sell_amount: to_wei(1001),
                buy_amount: to_wei(1000),
                kind: OrderKind::Sell,
                partially_fillable: false,
                settlement_handling: CapturingLimitOrderSettlementHandler::arc(),
            },
            LimitOrder {
                sell_token: token_b,
                buy_token: token_a,
                sell_amount: to_wei(1001),
                buy_amount: to_wei(1000),
                kind: OrderKind::Sell,
                partially_fillable: false,
                settlement_handling: CapturingLimitOrderSettlementHandler::arc(),
            },
        ];

        let amm_handler = CapturingAmmSettlementHandler::arc();
        let pool = AmmOrder {
            tokens: TokenPair::new(token_a, token_b).unwrap(),
            reserves: (to_wei(1_000_001).as_u128(), to_wei(1_000_000).as_u128()),
            fee: Rational::new(3, 1000),
            settlement_handling: amm_handler.clone(),
        };
        let result = solve(orders.into_iter(), &pool);
        assert!(amm_handler.calls().is_empty());
        assert_eq!(
            result.clearing_prices,
            maplit::hashmap! {
                token_a => to_wei(1_000_000),
                token_b => to_wei(1_000_001)
            }
        );
    }

    #[test]
    fn finds_solution_excluding_orders_whose_limit_price_is_not_satisfiable() {
        let token_a = Address::from_low_u64_be(0);
        let token_b = Address::from_low_u64_be(1);
        let orders = vec![
            // Unreasonable order a -> b
            OrderCreation {
                sell_token: token_a,
                buy_token: token_b,
                sell_amount: to_wei(1),
                buy_amount: to_wei(1000),
                kind: OrderKind::Sell,
                partially_fillable: false,
                ..Default::default()
            }
            .into(),
            // Reasonable order a -> b
            OrderCreation {
                sell_token: token_a,
                buy_token: token_b,
                sell_amount: to_wei(1000),
                buy_amount: to_wei(1000),
                kind: OrderKind::Sell,
                partially_fillable: false,
                ..Default::default()
            }
            .into(),
            // Reasonable order b -> a
            OrderCreation {
                sell_token: token_b,
                buy_token: token_a,
                sell_amount: to_wei(1000),
                buy_amount: to_wei(1000),
                kind: OrderKind::Sell,
                partially_fillable: false,
                ..Default::default()
            }
            .into(),
            // Unreasonable order b -> a
            OrderCreation {
                sell_token: token_b,
                buy_token: token_a,
                sell_amount: to_wei(2),
                buy_amount: to_wei(1000),
                kind: OrderKind::Sell,
                partially_fillable: false,
                ..Default::default()
            }
            .into(),
        ];

        let amm_handler = CapturingAmmSettlementHandler::arc();
        let pool = AmmOrder {
            tokens: TokenPair::new(token_a, token_b).unwrap(),
            reserves: (to_wei(1_000_000).as_u128(), to_wei(1_000_000).as_u128()),
            fee: Rational::new(3, 1000),
            settlement_handling: amm_handler,
        };
        let result = solve(orders.into_iter(), &pool);

        assert_eq!(result.trades.len(), 2);
        assert_eq!(is_valid_solution(&result), true);
    }

    #[test]
    fn returns_empty_solution_if_orders_have_no_overlap() {
        let token_a = Address::from_low_u64_be(0);
        let token_b = Address::from_low_u64_be(1);
        let orders = vec![
            LimitOrder {
                sell_token: token_a,
                buy_token: token_b,
                sell_amount: to_wei(900),
                buy_amount: to_wei(1000),
                kind: OrderKind::Sell,
                partially_fillable: false,
                settlement_handling: CapturingLimitOrderSettlementHandler::arc(),
            },
            LimitOrder {
                sell_token: token_b,
                buy_token: token_a,
                sell_amount: to_wei(900),
                buy_amount: to_wei(1000),
                kind: OrderKind::Sell,
                partially_fillable: false,
                settlement_handling: CapturingLimitOrderSettlementHandler::arc(),
            },
        ];

        let amm_handler = CapturingAmmSettlementHandler::arc();
        let pool = AmmOrder {
            tokens: TokenPair::new(token_a, token_b).unwrap(),
            reserves: (to_wei(1_000_001).as_u128(), to_wei(1_000_000).as_u128()),
            fee: Rational::new(3, 1000),
            settlement_handling: amm_handler,
        };
        let result = solve(orders.into_iter(), &pool);
        assert_eq!(result.trades.len(), 0);
    }

    #[test]
    fn test_is_valid_solution() {
        let token_a = Address::from_low_u64_be(0);
        let token_b = Address::from_low_u64_be(1);
        let orders = vec![
            OrderCreation {
                sell_token: token_a,
                buy_token: token_b,
                sell_amount: to_wei(10),
                buy_amount: to_wei(8),
                kind: OrderKind::Sell,
                partially_fillable: false,
                ..Default::default()
            },
            OrderCreation {
                sell_token: token_b,
                buy_token: token_a,
                sell_amount: to_wei(10),
                buy_amount: to_wei(9),
                kind: OrderKind::Sell,
                partially_fillable: false,
                ..Default::default()
            },
        ];

        // Price in the middle is ok
        assert_eq!(
            is_valid_solution(&Settlement {
                clearing_prices: maplit::hashmap! {
                    token_a => to_wei(1),
                    token_b => to_wei(1)
                },
                interactions: Vec::new(),
                trades: orders
                    .clone()
                    .into_iter()
                    .map(Trade::fully_matched)
                    .collect(),
                ..Default::default()
            }),
            true
        );

        // Price at the limit of first order is ok
        assert_eq!(
            is_valid_solution(&Settlement {
                clearing_prices: maplit::hashmap! {
                    token_a => to_wei(8),
                    token_b => to_wei(10)
                },
                interactions: Vec::new(),
                trades: orders
                    .clone()
                    .into_iter()
                    .map(Trade::fully_matched)
                    .collect(),
                ..Default::default()
            }),
            true
        );

        // Price at the limit of second order is ok
        assert_eq!(
            is_valid_solution(&Settlement {
                clearing_prices: maplit::hashmap! {
                    token_a => to_wei(10),
                    token_b => to_wei(9)
                },
                interactions: Vec::new(),
                trades: orders
                    .clone()
                    .into_iter()
                    .map(Trade::fully_matched)
                    .collect(),
                ..Default::default()
            }),
            true
        );

        // Price violating first order is not ok
        assert_eq!(
            is_valid_solution(&Settlement {
                clearing_prices: maplit::hashmap! {
                    token_a => to_wei(7),
                    token_b => to_wei(10)
                },
                interactions: Vec::new(),
                trades: orders
                    .clone()
                    .into_iter()
                    .map(Trade::fully_matched)
                    .collect(),
                ..Default::default()
            }),
            false
        );

        // Price violating second order is not ok
        assert_eq!(
            is_valid_solution(&Settlement {
                clearing_prices: maplit::hashmap! {
                    token_a => to_wei(10),
                    token_b => to_wei(8)
                },
                interactions: Vec::new(),
                trades: orders.into_iter().map(Trade::fully_matched).collect(),
                ..Default::default()
            }),
            false
        );
    }
}
