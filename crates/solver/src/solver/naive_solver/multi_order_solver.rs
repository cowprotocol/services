use crate::{liquidity, settlement::Settlement};
use anyhow::Result;
use liquidity::{AmmOrderExecution, ConstantProductOrder, LimitOrder};
use model::{
    order::{OrderData, OrderKind},
    TokenPair,
};
use num::{rational::Ratio, BigInt, BigRational, CheckedDiv};
use primitive_types::U256;
use shared::conversions::{
    big_int_to_u256, big_rational_to_u256, u256_to_big_int, RatioExt, U256Ext,
};
use std::collections::HashMap;
use web3::types::Address;

#[derive(Debug, Clone)]
struct TokenContext {
    address: Address,
    reserve: U256,
    buy_volume: U256,
    sell_volume: U256,
}

impl TokenContext {
    pub fn is_excess_after_fees(&self, deficit: &TokenContext, fee: Ratio<u32>) -> bool {
        fee.denom()
            * u256_to_big_int(&self.reserve)
            * (u256_to_big_int(&deficit.sell_volume) - u256_to_big_int(&deficit.buy_volume))
            < (fee.denom() - fee.numer())
                * u256_to_big_int(&deficit.reserve)
                * (u256_to_big_int(&self.sell_volume) - u256_to_big_int(&self.buy_volume))
    }

    pub fn is_excess_before_fees(&self, deficit: &TokenContext) -> bool {
        u256_to_big_int(&self.reserve)
            * (u256_to_big_int(&deficit.sell_volume) - u256_to_big_int(&deficit.buy_volume))
            < u256_to_big_int(&deficit.reserve)
                * (u256_to_big_int(&self.sell_volume) - u256_to_big_int(&self.buy_volume))
    }
}

pub fn solve(
    orders: impl IntoIterator<Item = LimitOrder>,
    pool: &ConstantProductOrder,
) -> Option<Settlement> {
    let mut orders: Vec<LimitOrder> = orders.into_iter().collect();
    while !orders.is_empty() {
        let (context_a, context_b) = split_into_contexts(&orders, pool);
        if let Some(valid_solution) = solve_orders(&orders, pool, &context_a, &context_b)
            .filter(|settlement| is_valid_solution(settlement, pool.tokens))
        {
            return Some(valid_solution);
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

    None
}

///
/// Computes a settlement using orders of a single pair and the direct AMM between those tokens.get(.
/// Panics if orders are not already filtered for a specific token pair, or the reserve information
/// for that pair is not available.
///
fn solve_orders(
    orders: &[LimitOrder],
    pool: &ConstantProductOrder,
    context_a: &TokenContext,
    context_b: &TokenContext,
) -> Option<Settlement> {
    if context_a.is_excess_after_fees(context_b, pool.fee) {
        solve_with_uniswap(orders, pool, context_b, context_a)
    } else if context_b.is_excess_after_fees(context_a, pool.fee) {
        solve_with_uniswap(orders, pool, context_a, context_b)
    } else {
        solve_without_uniswap(orders, context_a, context_b).ok()
    }
}

///
/// Creates a solution using the current AMM spot price, without using any of its liquidity
///
fn solve_without_uniswap(
    orders: &[LimitOrder],
    context_a: &TokenContext,
    context_b: &TokenContext,
) -> Result<Settlement> {
    let mut settlement = Settlement::new(maplit::hashmap! {
        context_a.address => context_b.reserve,
        context_b.address => context_a.reserve,
    });
    for order in orders {
        settlement.with_liquidity(order, order.full_execution_amount())?;
    }

    Ok(settlement)
}

///
/// Creates a solution using the current AMM's liquidity to balance excess and shortage.
/// The clearing price is the effective exchange rate used by the AMM interaction.
///
fn solve_with_uniswap(
    orders: &[LimitOrder],
    pool: &ConstantProductOrder,
    shortage: &TokenContext,
    excess: &TokenContext,
) -> Option<Settlement> {
    let uniswap_in_token = excess.address;
    let uniswap_out_token = shortage.address;

    let uniswap_out = compute_uniswap_out(shortage, excess, pool.fee)?;
    let uniswap_in = compute_uniswap_in(uniswap_out.clone(), shortage, excess, pool.fee)?;

    let uniswap_out = big_rational_to_u256(&uniswap_out).ok()?;
    let uniswap_in = big_rational_to_u256(&uniswap_in).ok()?;

    let mut settlement = Settlement::new(maplit::hashmap! {
        uniswap_in_token => uniswap_out,
        uniswap_out_token => uniswap_in,
    });
    for order in orders {
        settlement
            .with_liquidity(order, order.full_execution_amount())
            .ok()?;
    }

    // Because the smart contracts round in the favour of the traders, it could
    // be that we actually require a bit more from the Uniswap pool in order to
    // pay out all proceeds. We move the rounding error to the sell token so that
    // it either comes out of the fees or existing buffers. Compute that amount:
    let uniswap_out_with_rounding = big_int_to_u256(&settlement.executed_trades().fold(
        BigInt::default(),
        |mut total, (_, trade)| {
            if trade.sell_token == uniswap_out_token {
                total -= trade.sell_amount.to_big_int();
            } else {
                total += trade.buy_amount.to_big_int();
            };
            total
        },
    ))
    .ok()?;

    // In theory, not all limit orders are GPv2 trades (although in practice
    // they are). Since those orders don't generate trades, they aren't
    // considered in the `uniswap_out_with_rounding` computation. Luckily, they
    // don't get any surplus anyway, so we can just use the original output
    // amount as the rounding error will come from the surplus that those orders
    // would have otherwise received.
    // This only really matters for unit tests anyway ¯\_(ツ)_/¯.
    let uniswap_out_with_rounding = uniswap_out_with_rounding.max(uniswap_out);

    settlement
        .with_liquidity(
            pool,
            AmmOrderExecution {
                input: (uniswap_in_token, uniswap_in),
                output: (uniswap_out_token, uniswap_out_with_rounding),
            },
        )
        .ok()?;

    Some(settlement)
}

impl ConstantProductOrder {
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
    orders: &[LimitOrder],
    pool: &ConstantProductOrder,
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
    assert_eq!(contexts.len(), 2, "Orders contain more than two tokens");
    let mut contexts = contexts.drain().map(|(_, v)| v);
    (contexts.next().unwrap(), contexts.next().unwrap())
}

///
/// Given information about the shortage token (the one we need to take from Uniswap) and the excess token (the one we give to Uniswap), this function
/// computes the exact out_amount required from Uniswap to perfectly match demand and supply at the effective Uniswap price (the one used for that in/out swap).
///
/// The derivation of this formula is described in https://docs.google.com/document/d/1jS22wxbCqo88fGsqEMZgRQgiAcHlPqxoMw3CJTHst6c/edit
/// It assumes GP fee (φ) to be 1
///
fn compute_uniswap_out(
    shortage: &TokenContext,
    excess: &TokenContext,
    amm_fee: Ratio<u32>,
) -> Option<BigRational> {
    let numerator_minuend = (amm_fee.denom() - amm_fee.numer())
        * (u256_to_big_int(&excess.sell_volume) - u256_to_big_int(&excess.buy_volume))
        * u256_to_big_int(&shortage.reserve);
    let numerator_subtrahend = amm_fee.denom()
        * (u256_to_big_int(&shortage.sell_volume) - u256_to_big_int(&shortage.buy_volume))
        * u256_to_big_int(&excess.reserve);
    let denominator: BigInt = amm_fee.denom() * u256_to_big_int(&excess.reserve)
        + (amm_fee.denom() - amm_fee.numer())
            * (u256_to_big_int(&excess.sell_volume) - u256_to_big_int(&excess.buy_volume));
    BigRational::new_checked(numerator_minuend - numerator_subtrahend, denominator).ok()
}

///
/// Given the desired amount to receive and the state of the pool, this computes the required amount
/// of tokens to be sent to the pool.
/// Taken from: https://github.com/Uniswap/uniswap-v2-periphery/blob/4123f93278b60bcf617130629c69d4016f9e7584/contracts/libraries/UniswapV2Library.sol#L53
/// Not adding + 1 in the end, because we are working with rationals and thus don't round up.
///
fn compute_uniswap_in(
    out: BigRational,
    shortage: &TokenContext,
    excess: &TokenContext,
    amm_fee: Ratio<u32>,
) -> Option<BigRational> {
    let numerator = U256::from(*amm_fee.denom()).to_big_rational()
        * out.clone()
        * u256_to_big_int(&excess.reserve);
    let denominator = U256::from(amm_fee.denom() - amm_fee.numer()).to_big_rational()
        * (shortage.reserve.to_big_rational() - out);
    numerator.checked_div(&denominator)
}

///
/// Returns true if for each trade the executed price is not smaller than the limit price
/// Thus we ensure that `buy_token_price / sell_token_price >= limit_buy_amount / limit_sell_amount`
///
fn is_valid_solution(solution: &Settlement, pair: TokenPair) -> bool {
    // We can't rely on the buy token having a price for liquidity orders that
    // buy Eth. This is because no "token equivalency" clearing price gets added
    // for these orders, and their clearing buy price only gets added at when
    // actually building the settlement calldata. Instead, we use the token pair
    // to find and return the address of the token that is not the order's sell
    // token.
    let mapped_buy_token = |order: &OrderData| {
        let (token0, token1) = pair.get();
        if order.sell_token == token0 {
            token1
        } else {
            token0
        }
    };

    for order in solution.traded_orders() {
        let order = &order.data;
        let buy_token_price = solution
            .clearing_price(mapped_buy_token(order))
            .expect("Solution should contain clearing price for mapped buy token");
        let sell_token_price = solution
            .clearing_price(order.sell_token)
            .expect("Solution should contain clearing price for sell token");

        match (
            order.sell_amount.checked_mul(sell_token_price),
            order.buy_amount.checked_mul(buy_token_price),
        ) {
            (Some(sell_volume), Some(buy_volume)) if sell_volume >= buy_volume => (),
            _ => return false,
        }
    }

    true
}

#[cfg(test)]
mod tests {
    use liquidity::tests::CapturingSettlementHandler;
    use maplit::hashmap;
    use model::{
        order::{Order, OrderData},
        TokenPair,
    };
    use num::rational::Ratio;
    use shared::{baseline_solver::BaselineSolvable, sources::uniswap_v2::pool_fetching::Pool};

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
                id: "0".to_string(),
                ..Default::default()
            },
            LimitOrder {
                sell_token: token_b,
                buy_token: token_a,
                sell_amount: to_wei(100),
                buy_amount: to_wei(90),
                kind: OrderKind::Sell,
                id: "1".to_string(),
                ..Default::default()
            },
        ];

        let amm_handler = CapturingSettlementHandler::arc();
        let pool = ConstantProductOrder {
            tokens: TokenPair::new(token_a, token_b).unwrap(),
            reserves: (to_wei(1000).as_u128(), to_wei(1000).as_u128()),
            fee: Ratio::new(3, 1000),
            settlement_handling: amm_handler.clone(),
        };
        let result = solve(orders.clone(), &pool).unwrap();

        // Make sure the uniswap interaction is using the correct direction
        let interaction = amm_handler.calls()[0].clone();
        assert_eq!(interaction.input.0, token_b);
        assert_eq!(interaction.output.0, token_a);

        // Make sure the sell amounts +/- uniswap interaction satisfy min_buy amounts
        assert!(orders[0].sell_amount + interaction.output.1 >= orders[1].buy_amount);
        assert!(orders[1].sell_amount - interaction.input.1 > orders[0].buy_amount);

        // Make sure the sell amounts +/- uniswap interaction satisfy expected buy amounts given clearing price
        let price_a = result.clearing_price(token_a).unwrap();
        let price_b = result.clearing_price(token_b).unwrap();

        // Multiplying sellAmount with priceA, gives us sell value in "$", divided by priceB gives us value in buy token
        // We should have at least as much to give (sell amount +/- uniswap) as is expected by the buyer
        let expected_buy = (orders[0].sell_amount * price_a).ceil_div(&price_b);
        assert!(orders[1].sell_amount - interaction.input.1 >= expected_buy);

        let expected_buy = (orders[1].sell_amount * price_b).ceil_div(&price_a);
        assert!(orders[0].sell_amount + interaction.input.1 >= expected_buy);
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
                id: "0".to_string(),
                ..Default::default()
            },
            LimitOrder {
                sell_token: token_a,
                buy_token: token_b,
                sell_amount: to_wei(100),
                buy_amount: to_wei(90),
                kind: OrderKind::Sell,
                id: "1".to_string(),
                ..Default::default()
            },
        ];

        let amm_handler = CapturingSettlementHandler::arc();
        let pool = ConstantProductOrder {
            tokens: TokenPair::new(token_a, token_b).unwrap(),
            reserves: (to_wei(1_000_000).as_u128(), to_wei(1_000_000).as_u128()),
            fee: Ratio::new(3, 1000),
            settlement_handling: amm_handler.clone(),
        };
        let result = solve(orders.clone(), &pool).unwrap();

        // Make sure the uniswap interaction is using the correct direction
        let interaction = amm_handler.calls()[0].clone();
        assert_eq!(interaction.input.0, token_a);
        assert_eq!(interaction.output.0, token_b);

        // Make sure the sell amounts cover the uniswap in, and min buy amounts are covered by uniswap out
        assert!(orders[0].sell_amount + orders[1].sell_amount >= interaction.input.1);
        assert!(interaction.output.1 >= orders[0].buy_amount + orders[1].buy_amount);

        // Make sure expected buy amounts (given prices) are also covered by uniswap out amounts
        let price_a = result.clearing_price(token_a).unwrap();
        let price_b = result.clearing_price(token_b).unwrap();

        let first_expected_buy = orders[0].sell_amount * price_a / price_b;
        let second_expected_buy = orders[1].sell_amount * price_a / price_b;
        assert!(interaction.output.1 >= first_expected_buy + second_expected_buy);
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
                id: "0".to_string(),
                ..Default::default()
            },
            LimitOrder {
                sell_token: token_b,
                buy_token: token_a,
                sell_amount: to_wei(100),
                buy_amount: to_wei(90),
                kind: OrderKind::Buy,
                id: "1".to_string(),
                ..Default::default()
            },
        ];

        let amm_handler = CapturingSettlementHandler::arc();
        let pool = ConstantProductOrder {
            tokens: TokenPair::new(token_a, token_b).unwrap(),
            reserves: (to_wei(1000).as_u128(), to_wei(1000).as_u128()),
            fee: Ratio::new(3, 1000),
            settlement_handling: amm_handler.clone(),
        };
        let result = solve(orders.clone(), &pool).unwrap();

        // Make sure the uniswap interaction is using the correct direction
        let interaction = amm_handler.calls()[0].clone();
        assert_eq!(interaction.input.0, token_b);
        assert_eq!(interaction.output.0, token_a);

        // Make sure the buy amounts +/- uniswap interaction satisfy max_sell amounts
        assert!(orders[0].sell_amount >= orders[1].buy_amount - interaction.output.1);
        assert!(orders[1].sell_amount >= orders[0].buy_amount + interaction.input.1);

        // Make sure buy sell amounts +/- uniswap interaction satisfy expected sell amounts given clearing price
        let price_a = result.clearing_price(token_a).unwrap();
        let price_b = result.clearing_price(token_b).unwrap();

        // Multiplying buyAmount with priceB, gives us sell value in "$", divided by priceA gives us value in sell token
        // The seller should expect to sell at least as much as we require for the buyer + uniswap.
        let expected_sell = orders[0].buy_amount * price_b / price_a;
        assert!(orders[1].buy_amount - interaction.input.1 <= expected_sell);

        let expected_sell = orders[1].buy_amount * price_a / price_b;
        assert!(orders[0].buy_amount + interaction.output.1 <= expected_sell);
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
                id: "0".to_string(),
                ..Default::default()
            },
            LimitOrder {
                sell_token: token_b,
                buy_token: token_a,
                sell_amount: to_wei(100),
                buy_amount: to_wei(90),
                kind: OrderKind::Sell,
                id: "1".to_string(),
                ..Default::default()
            },
        ];

        let amm_handler = CapturingSettlementHandler::arc();
        let pool = ConstantProductOrder {
            tokens: TokenPair::new(token_a, token_b).unwrap(),
            reserves: (to_wei(1000).as_u128(), to_wei(1000).as_u128()),
            fee: Ratio::new(3, 1000),
            settlement_handling: amm_handler.clone(),
        };
        let result = solve(orders.clone(), &pool).unwrap();

        // Make sure the uniswap interaction is using the correct direction
        let interaction = amm_handler.calls()[0].clone();
        assert_eq!(interaction.input.0, token_b);
        assert_eq!(interaction.output.0, token_a);

        // Make sure the buy order's sell amount - uniswap interaction satisfies sell order's limit
        assert!(orders[0].sell_amount >= orders[1].buy_amount - interaction.output.1);

        // Make sure the sell order's buy amount + uniswap interaction satisfies buy order's limit
        assert!(orders[1].buy_amount + interaction.input.1 >= orders[0].sell_amount);

        // Make sure buy sell amounts +/- uniswap interaction satisfy expected sell amounts given clearing price
        let price_a = result.clearing_price(token_a).unwrap();
        let price_b = result.clearing_price(token_b).unwrap();

        // Multiplying buy_amount with priceB, gives us sell value in "$", divided by priceA gives us value in sell token
        // The seller should expect to sell at least as much as we require for the buyer + uniswap.
        let expected_sell = orders[0].buy_amount * price_b / price_a;
        assert!(orders[1].buy_amount - interaction.input.1 <= expected_sell);

        // Multiplying sell_amount with priceA, gives us sell value in "$", divided by priceB gives us value in buy token
        // We should have at least as much to give (sell amount + uniswap out) as is expected by the buyer
        let expected_buy = orders[1].sell_amount * price_b / price_a;
        assert!(orders[0].sell_amount + interaction.output.1 >= expected_buy);
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
                id: "0".to_string(),
                ..Default::default()
            },
            LimitOrder {
                sell_token: token_b,
                buy_token: token_a,
                sell_amount: to_wei(1001),
                buy_amount: to_wei(1000),
                kind: OrderKind::Sell,
                id: "1".to_string(),
                ..Default::default()
            },
        ];

        let amm_handler = CapturingSettlementHandler::arc();
        let pool = ConstantProductOrder {
            tokens: TokenPair::new(token_a, token_b).unwrap(),
            reserves: (to_wei(1_000_001).as_u128(), to_wei(1_000_000).as_u128()),
            fee: Ratio::new(3, 1000),
            settlement_handling: amm_handler.clone(),
        };
        let result = solve(orders, &pool).unwrap();
        assert!(amm_handler.calls().is_empty());
        assert_eq!(
            result.clearing_prices(),
            &maplit::hashmap! {
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
            Order {
                data: OrderData {
                    sell_token: token_a,
                    buy_token: token_b,
                    sell_amount: to_wei(1),
                    buy_amount: to_wei(1000),
                    kind: OrderKind::Sell,
                    partially_fillable: false,
                    ..Default::default()
                },
                ..Default::default()
            }
            .into(),
            // Reasonable order a -> b
            Order {
                data: OrderData {
                    sell_token: token_a,
                    buy_token: token_b,
                    sell_amount: to_wei(1000),
                    buy_amount: to_wei(1000),
                    kind: OrderKind::Sell,
                    partially_fillable: false,
                    ..Default::default()
                },
                ..Default::default()
            }
            .into(),
            // Reasonable order b -> a
            Order {
                data: OrderData {
                    sell_token: token_b,
                    buy_token: token_a,
                    sell_amount: to_wei(1000),
                    buy_amount: to_wei(1000),
                    kind: OrderKind::Sell,
                    partially_fillable: false,
                    ..Default::default()
                },
                ..Default::default()
            }
            .into(),
            // Unreasonable order b -> a
            Order {
                data: OrderData {
                    sell_token: token_b,
                    buy_token: token_a,
                    sell_amount: to_wei(2),
                    buy_amount: to_wei(1000),
                    kind: OrderKind::Sell,
                    partially_fillable: false,
                    ..Default::default()
                },
                ..Default::default()
            }
            .into(),
        ];

        let amm_handler = CapturingSettlementHandler::arc();
        let pool = ConstantProductOrder {
            tokens: TokenPair::new(token_a, token_b).unwrap(),
            reserves: (to_wei(1_000_000).as_u128(), to_wei(1_000_000).as_u128()),
            fee: Ratio::new(3, 1000),
            settlement_handling: amm_handler,
        };
        let result = solve(orders, &pool).unwrap();

        assert_eq!(result.traded_orders().count(), 2);
        assert!(is_valid_solution(&result, pool.tokens));
    }

    #[test]
    fn returns_empty_solution_if_orders_have_no_overlap() {
        let token_a = Address::from_low_u64_be(0);
        let token_b = Address::from_low_u64_be(1);
        let orders = vec![
            Order {
                data: OrderData {
                    sell_token: token_a,
                    buy_token: token_b,
                    sell_amount: to_wei(900),
                    buy_amount: to_wei(1000),
                    kind: OrderKind::Sell,
                    partially_fillable: false,
                    ..Default::default()
                },
                ..Default::default()
            }
            .into(),
            Order {
                data: OrderData {
                    sell_token: token_b,
                    buy_token: token_a,
                    sell_amount: to_wei(900),
                    buy_amount: to_wei(1000),
                    kind: OrderKind::Sell,
                    partially_fillable: false,
                    ..Default::default()
                },
                ..Default::default()
            }
            .into(),
        ];

        let amm_handler = CapturingSettlementHandler::arc();
        let pool = ConstantProductOrder {
            tokens: TokenPair::new(token_a, token_b).unwrap(),
            reserves: (to_wei(1_000_001).as_u128(), to_wei(1_000_000).as_u128()),
            fee: Ratio::new(3, 1000),
            settlement_handling: amm_handler,
        };
        assert!(solve(orders, &pool).is_none());
    }

    #[test]
    fn test_is_valid_solution() {
        let token_a = Address::from_low_u64_be(0);
        let token_b = Address::from_low_u64_be(1);
        let tokens = TokenPair::new(token_a, token_b).unwrap();
        let orders = vec![
            Order {
                data: OrderData {
                    sell_token: token_a,
                    buy_token: token_b,
                    sell_amount: to_wei(10),
                    buy_amount: to_wei(8),
                    kind: OrderKind::Sell,
                    partially_fillable: false,
                    ..Default::default()
                },
                ..Default::default()
            },
            Order {
                data: OrderData {
                    sell_token: token_b,
                    buy_token: token_a,
                    sell_amount: to_wei(10),
                    buy_amount: to_wei(9),
                    kind: OrderKind::Sell,
                    partially_fillable: false,
                    ..Default::default()
                },
                ..Default::default()
            },
        ];

        let settlement_with_prices = |prices: HashMap<Address, U256>| {
            let mut settlement = Settlement::new(prices);
            for order in orders.iter().cloned() {
                let limit_order = LimitOrder::from(order);
                settlement
                    .with_liquidity(&limit_order, limit_order.full_execution_amount())
                    .unwrap();
            }
            settlement
        };

        // Price in the middle is ok
        assert!(is_valid_solution(
            &settlement_with_prices(maplit::hashmap! {
                token_a => to_wei(1),
                token_b => to_wei(1)
            }),
            tokens
        ));

        // Price at the limit of first order is ok
        assert!(is_valid_solution(
            &settlement_with_prices(maplit::hashmap! {
                token_a => to_wei(8),
                token_b => to_wei(10)
            }),
            tokens
        ));

        // Price at the limit of second order is ok
        assert!(is_valid_solution(
            &settlement_with_prices(maplit::hashmap! {
                token_a => to_wei(10),
                token_b => to_wei(9)
            }),
            tokens
        ));

        // Price violating first order is not ok
        assert!(!is_valid_solution(
            &settlement_with_prices(maplit::hashmap! {
                token_a => to_wei(7),
                token_b => to_wei(10)
            }),
            tokens
        ));

        // Price violating second order is not ok
        assert!(!is_valid_solution(
            &settlement_with_prices(maplit::hashmap! {
                token_a => to_wei(10),
                token_b => to_wei(8)
            }),
            tokens
        ));
    }

    #[test]
    fn does_not_panic() {
        let token_a = Address::from_low_u64_be(0);
        let token_b = Address::from_low_u64_be(1);
        let orders = vec![
            Order {
                data: OrderData {
                    sell_token: token_a,
                    buy_token: token_b,
                    sell_amount: U256::MAX,
                    buy_amount: 1.into(),
                    kind: OrderKind::Sell,
                    partially_fillable: false,
                    ..Default::default()
                },
                ..Default::default()
            }
            .into(),
            Order {
                data: OrderData {
                    sell_token: token_b,
                    buy_token: token_a,
                    sell_amount: 1.into(),
                    buy_amount: 1.into(),
                    kind: OrderKind::Sell,
                    partially_fillable: false,
                    ..Default::default()
                },
                ..Default::default()
            }
            .into(),
        ];

        let amm_handler = CapturingSettlementHandler::arc();
        let pool = ConstantProductOrder {
            tokens: TokenPair::new(token_a, token_b).unwrap(),
            reserves: (u128::MAX, u128::MAX),
            fee: Ratio::new(3, 1000),
            settlement_handling: amm_handler,
        };
        // This line should not panic.
        solve(orders, &pool);
    }

    #[test]
    fn reserves_are_too_small() {
        let token_a = Address::from_low_u64_be(0);
        let token_b = Address::from_low_u64_be(1);
        let orders = vec![
            Order {
                data: OrderData {
                    sell_token: token_a,
                    buy_token: token_b,
                    sell_amount: 70145218378783248142575u128.into(),
                    buy_amount: 70123226323u128.into(),
                    kind: OrderKind::Sell,
                    partially_fillable: false,
                    ..Default::default()
                },
                ..Default::default()
            }
            .into(),
            Order {
                data: OrderData {
                    sell_token: token_a,
                    buy_token: token_b,
                    sell_amount: 900_000_000_000_000u128.into(),
                    buy_amount: 100.into(),
                    kind: OrderKind::Sell,
                    partially_fillable: false,
                    ..Default::default()
                },
                ..Default::default()
            }
            .into(),
        ];
        // Reserves are much smaller than buy amount
        let pool = ConstantProductOrder {
            tokens: TokenPair::new(token_a, token_b).unwrap(),
            reserves: (25000075, 2500007500),
            fee: Ratio::new(3, 1000),
            settlement_handling: CapturingSettlementHandler::arc(),
        };

        // The first order by itself should not be matchable.
        assert!(solve(orders[0..1].to_vec(), &pool).is_none());

        // Only the second order should match
        let result = solve(orders, &pool).unwrap();
        assert_eq!(result.traded_orders().count(), 1);
    }

    #[test]
    fn rounds_prices_in_favour_of_traders() {
        let token_a = Address::from_low_u64_be(0);
        let token_b = Address::from_low_u64_be(1);
        let orders = vec![
            Order {
                data: OrderData {
                    sell_token: token_a,
                    buy_token: token_b,
                    sell_amount: 9_000_000.into(),
                    buy_amount: 8_500_000.into(),
                    kind: OrderKind::Sell,
                    partially_fillable: false,
                    ..Default::default()
                },
                ..Default::default()
            }
            .into(),
            Order {
                data: OrderData {
                    buy_token: token_a,
                    sell_token: token_b,
                    buy_amount: 8_000_001.into(),
                    sell_amount: 8_500_000.into(),
                    kind: OrderKind::Buy,
                    partially_fillable: false,
                    ..Default::default()
                },
                ..Default::default()
            }
            .into(),
        ];

        let pool = Pool::uniswap(
            TokenPair::new(token_a, token_b).unwrap(),
            (1_000_000_000_000_000_000, 1_000_000_000_000_000_000),
        );

        // Note that these orders have an excess of ~1e6 T_a because the first
        // order is a sell order selling 9e6 T_a and the second order is buying
        // 8e6 T_a. Use this to compute the exact amount that will get swapped
        // with the pool. This will define our prices.
        let excess_amount_a = 999_999.into();
        let swapped_amount_b = pool
            .get_amount_out(token_b, (excess_amount_a, token_a))
            .unwrap();

        // The naive solver sets the prices to the the swapped amounts, so:
        let expected_prices = hashmap! {
            token_a => swapped_amount_b,
            token_b => excess_amount_a,
        };

        let settlement = solve(orders, &pool.into()).unwrap();
        let trades = settlement
            .executed_trades()
            .map(|(_, trade)| trade)
            .collect::<Vec<_>>();

        // Check the prices are set according to the expected pool swap:
        assert_eq!(settlement.clearing_prices(), &expected_prices);

        // Check the executed buy amount gets rounded up for the sell order:
        assert_eq!(
            trades[0].buy_amount,
            U256::from(9_000_000) * expected_prices[&token_a] / expected_prices[&token_b] + 1,
        );

        // Check the executed sell amount gets rounded down for the buy order:
        assert_eq!(
            trades[1].sell_amount,
            U256::from(8_000_001) * expected_prices[&token_a] / expected_prices[&token_b],
        );
    }
}
