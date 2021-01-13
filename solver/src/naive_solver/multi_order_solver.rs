use super::single_pair_settlement::{AmmSwapExactTokensForTokens, SinglePairSettlement};
use crate::{settlement::Trade, uniswap::Pool};
use anyhow::{anyhow, Result};
use model::order::{OrderCreation, OrderKind};
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

impl TokenContext {
    pub fn is_excess_after_fees(&self, deficit: &TokenContext) -> bool {
        1000 * u256_to_bigint(&self.reserve)
            * (u256_to_bigint(&deficit.sell_volume) - u256_to_bigint(&deficit.buy_volume))
            < 997
                * u256_to_bigint(&deficit.reserve)
                * (u256_to_bigint(&self.sell_volume) - u256_to_bigint(&self.buy_volume))
    }
}

///
/// Computes a settlement using orders of a single pair and the direct AMM between those tokens.
/// Panics if orders are not already filtered for a specific token pair, or the reserve information
/// for that pair is not available.
///
pub fn solve(
    orders: impl Iterator<Item = OrderCreation> + Clone,
    pool: &Pool,
) -> SinglePairSettlement {
    let (context_a, context_b) = split_into_contexts(orders.clone(), pool);
    if context_a.is_excess_after_fees(&context_b) {
        solve_with_uniswap(orders, &context_b, &context_a)
    } else if context_b.is_excess_after_fees(&context_a) {
        solve_with_uniswap(orders, &context_a, &context_b)
    } else {
        solve_without_uniswap(orders, &context_a, &context_b)
    }
}

///
/// Creates a solution using the current AMM spot price, without using any of its liquidity
///
fn solve_without_uniswap(
    orders: impl Iterator<Item = OrderCreation> + Clone,
    context_a: &TokenContext,
    context_b: &TokenContext,
) -> SinglePairSettlement {
    SinglePairSettlement {
        clearing_prices: maplit::hashmap! {
            context_a.address => context_b.reserve,
            context_b.address => context_a.reserve,
        },
        // TODO(fleupold) check that all orders comply with the computed price. Otherwise, remove the least favorable excess order and try again.
        trades: orders.into_iter().map(Trade::fully_matched).collect(),
        interaction: None,
    }
}

///
/// Creates a solution using the current AMM's liquidity to balance excess and shortage.
/// The clearing price is the effective exchange rate used by the AMM interaction.
///
fn solve_with_uniswap(
    orders: impl Iterator<Item = OrderCreation> + Clone,
    shortage: &TokenContext,
    excess: &TokenContext,
) -> SinglePairSettlement {
    let uniswap_out = compute_uniswap_out(&shortage, &excess);
    let uniswap_in = compute_uniswap_in(uniswap_out, &shortage, &excess);
    let interaction = Some(AmmSwapExactTokensForTokens {
        amount_in: uniswap_in,
        amount_out_min: uniswap_out,
        token_in: excess.address,
        token_out: shortage.address,
    });
    // TODO(fleupold) check that all orders comply with the computed price. Otherwise, remove the least favorable excess order and try again.
    SinglePairSettlement {
        clearing_prices: maplit::hashmap! {
            shortage.address => uniswap_in,
            excess.address => uniswap_out,
        },
        trades: orders.into_iter().map(Trade::fully_matched).collect(),
        interaction,
    }
}

fn split_into_contexts(
    orders: impl Iterator<Item = OrderCreation>,
    pool: &Pool,
) -> (TokenContext, TokenContext) {
    let mut contexts = HashMap::new();
    for order in orders {
        let buy_context = contexts
            .entry(order.buy_token)
            .or_insert_with(|| TokenContext {
                address: order.buy_token,
                reserve: pool
                    .get_reserve(&order.buy_token)
                    .unwrap_or_else(|| panic!("No reserve for token {}", &order.buy_token))
                    .into(),
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
                    .unwrap_or_else(|| panic!("No reserve for token {}", &order.sell_token))
                    .into(),
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
    use model::TokenPair;

    use super::*;

    fn to_wei(base: u128) -> U256 {
        U256::from(base) * U256::from(10).pow(18.into())
    }

    #[test]
    fn finds_clearing_price_with_sell_orders_on_both_sides() {
        let token_a = Address::from_low_u64_be(0);
        let token_b = Address::from_low_u64_be(1);
        let orders = vec![
            OrderCreation {
                sell_token: token_a,
                buy_token: token_b,
                sell_amount: to_wei(40),
                buy_amount: to_wei(30),
                kind: OrderKind::Sell,
                partially_fillable: false,
                ..Default::default()
            },
            OrderCreation {
                sell_token: token_b,
                buy_token: token_a,
                sell_amount: to_wei(100),
                buy_amount: to_wei(90),
                kind: OrderKind::Sell,
                partially_fillable: false,
                ..Default::default()
            },
        ];

        let pool = Pool {
            token_pair: TokenPair::new(token_a, token_b).unwrap(),
            reserve0: to_wei(1000).as_u128(),
            reserve1: to_wei(1000).as_u128(),
            address: Default::default(),
        };
        let result = solve(orders.clone().into_iter(), &pool);

        // Make sure the uniswap interaction is using the correct direction
        let interaction = result.interaction.unwrap();
        assert_eq!(interaction.token_in, token_b);
        assert_eq!(interaction.token_out, token_a);

        // Make sure the sell amounts +/- uniswap interaction satisfy min_buy amounts
        assert!(orders[0].sell_amount + interaction.amount_out_min >= orders[1].buy_amount);
        assert!(orders[1].sell_amount - interaction.amount_in > orders[0].buy_amount);

        // Make sure the sell amounts +/- uniswap interaction satisfy expected buy amounts given clearing price
        let price_a = result.clearing_prices.get(&token_a).unwrap();
        let price_b = result.clearing_prices.get(&token_b).unwrap();

        // Multiplying sellAmount with priceA, gives us sell value in "$", divided by priceB gives us value in buy token
        // We should have at least as much to give (sell amount +/- uniswap) as is expected by the buyer
        let expected_buy = orders[0].sell_amount * price_a / price_b;
        assert!(orders[1].sell_amount - interaction.amount_in >= expected_buy);

        let expected_buy = orders[1].sell_amount * price_b / price_a;
        assert!(orders[0].sell_amount + interaction.amount_out_min >= expected_buy);
    }

    #[test]
    fn finds_clearing_price_with_sell_orders_on_one_side() {
        let token_a = Address::from_low_u64_be(0);
        let token_b = Address::from_low_u64_be(1);
        let orders = vec![
            OrderCreation {
                sell_token: token_a,
                buy_token: token_b,
                sell_amount: to_wei(40),
                buy_amount: to_wei(30),
                kind: OrderKind::Sell,
                partially_fillable: false,
                ..Default::default()
            },
            OrderCreation {
                sell_token: token_a,
                buy_token: token_b,
                sell_amount: to_wei(100),
                buy_amount: to_wei(90),
                kind: OrderKind::Sell,
                partially_fillable: false,
                ..Default::default()
            },
        ];

        let pool = Pool {
            token_pair: TokenPair::new(token_a, token_b).unwrap(),
            reserve0: to_wei(1000).as_u128(),
            reserve1: to_wei(1000).as_u128(),
            address: Default::default(),
        };
        let result = solve(orders.clone().into_iter(), &pool);

        // Make sure the uniswap interaction is using the correct direction
        let interaction = result.interaction.unwrap();
        assert_eq!(interaction.token_in, token_a);
        assert_eq!(interaction.token_out, token_b);

        // Make sure the sell amounts cover the uniswap in, and min buy amounts are covered by uniswap out
        assert!(orders[0].sell_amount + orders[1].sell_amount >= interaction.amount_in);
        assert!(interaction.amount_out_min >= orders[0].buy_amount + orders[1].buy_amount);

        // Make sure expected buy amounts (given prices) are also covered by uniswap out amounts
        let price_a = result.clearing_prices.get(&token_a).unwrap();
        let price_b = result.clearing_prices.get(&token_b).unwrap();

        let first_expected_buy = orders[0].sell_amount * price_a / price_b;
        let second_expected_buy = orders[1].sell_amount * price_a / price_b;
        assert!(interaction.amount_out_min >= first_expected_buy + second_expected_buy);
    }

    #[test]
    fn finds_clearing_price_with_buy_orders_on_both_sides() {
        let token_a = Address::from_low_u64_be(0);
        let token_b = Address::from_low_u64_be(1);
        let orders = vec![
            OrderCreation {
                sell_token: token_a,
                buy_token: token_b,
                sell_amount: to_wei(40),
                buy_amount: to_wei(30),
                kind: OrderKind::Buy,
                partially_fillable: false,
                ..Default::default()
            },
            OrderCreation {
                sell_token: token_b,
                buy_token: token_a,
                sell_amount: to_wei(100),
                buy_amount: to_wei(90),
                kind: OrderKind::Buy,
                partially_fillable: false,
                ..Default::default()
            },
        ];

        let pool = Pool {
            token_pair: TokenPair::new(token_a, token_b).unwrap(),
            reserve0: to_wei(1000).as_u128(),
            reserve1: to_wei(1000).as_u128(),
            address: Default::default(),
        };
        let result = solve(orders.clone().into_iter(), &pool);

        // Make sure the uniswap interaction is using the correct direction
        let interaction = result.interaction.unwrap();
        assert_eq!(interaction.token_in, token_b);
        assert_eq!(interaction.token_out, token_a);

        // Make sure the buy amounts +/- uniswap interaction satisfy max_sell amounts
        assert!(orders[0].sell_amount >= orders[1].buy_amount - interaction.amount_out_min);
        assert!(orders[1].sell_amount >= orders[0].buy_amount + interaction.amount_in);

        // Make sure buy sell amounts +/- uniswap interaction satisfy expected sell amounts given clearing price
        let price_a = result.clearing_prices.get(&token_a).unwrap();
        let price_b = result.clearing_prices.get(&token_b).unwrap();

        // Multiplying buyAmount with priceB, gives us sell value in "$", divided by priceA gives us value in sell token
        // The seller should expect to sell at least as much as we require for the buyer + uniswap.
        let expected_sell = orders[0].buy_amount * price_b / price_a;
        assert!(orders[1].buy_amount - interaction.amount_in <= expected_sell);

        let expected_sell = orders[1].buy_amount * price_a / price_b;
        assert!(orders[0].buy_amount + interaction.amount_out_min <= expected_sell);
    }

    #[test]
    fn finds_clearing_price_with_buy_orders_and_sell_orders() {
        let token_a = Address::from_low_u64_be(0);
        let token_b = Address::from_low_u64_be(1);
        let orders = vec![
            OrderCreation {
                sell_token: token_a,
                buy_token: token_b,
                sell_amount: to_wei(40),
                buy_amount: to_wei(30),
                kind: OrderKind::Buy,
                partially_fillable: false,
                ..Default::default()
            },
            OrderCreation {
                sell_token: token_b,
                buy_token: token_a,
                sell_amount: to_wei(100),
                buy_amount: to_wei(90),
                kind: OrderKind::Sell,
                partially_fillable: false,
                ..Default::default()
            },
        ];

        let pool = Pool {
            token_pair: TokenPair::new(token_a, token_b).unwrap(),
            reserve0: to_wei(1000).as_u128(),
            reserve1: to_wei(1000).as_u128(),
            address: Default::default(),
        };
        let result = solve(orders.clone().into_iter(), &pool);

        // Make sure the uniswap interaction is using the correct direction
        let interaction = result.interaction.unwrap();
        assert_eq!(interaction.token_in, token_b);
        assert_eq!(interaction.token_out, token_a);

        // Make sure the buy order's sell amount - uniswap interaction satisfies sell order's limit
        assert!(orders[0].sell_amount >= orders[1].buy_amount - interaction.amount_out_min);

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
        assert!(orders[0].sell_amount + interaction.amount_out_min >= expected_buy);
    }

    #[test]
    fn finds_clearing_without_using_uniswap() {
        let token_a = Address::from_low_u64_be(0);
        let token_b = Address::from_low_u64_be(1);
        let orders = vec![
            OrderCreation {
                sell_token: token_a,
                buy_token: token_b,
                sell_amount: to_wei(1001),
                buy_amount: to_wei(1000),
                kind: OrderKind::Sell,
                partially_fillable: false,
                ..Default::default()
            },
            OrderCreation {
                sell_token: token_b,
                buy_token: token_a,
                sell_amount: to_wei(1000),
                buy_amount: to_wei(1000),
                kind: OrderKind::Sell,
                partially_fillable: false,
                ..Default::default()
            },
        ];

        let pool = Pool {
            token_pair: TokenPair::new(token_a, token_b).unwrap(),
            reserve0: to_wei(1_000_001).as_u128(),
            reserve1: to_wei(1_000_000).as_u128(),
            address: Default::default(),
        };
        let result = solve(orders.into_iter(), &pool);
        assert_eq!(result.interaction, None);
        assert_eq!(
            result.clearing_prices,
            maplit::hashmap! {
                token_a => to_wei(1_000_000),
                token_b => to_wei(1_000_001)
            }
        );
    }
}
