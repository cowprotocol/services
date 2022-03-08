pub mod external_prices;
mod settlement_encoder;

use self::external_prices::ExternalPrices;
pub use self::settlement_encoder::SettlementEncoder;
use crate::{
    encoding::{self, EncodedInteraction, EncodedSettlement, EncodedTrade},
    liquidity::Settleable,
};
use anyhow::Result;
use model::order::{Order, OrderKind};
use num::{BigRational, One, Signed, Zero};
use primitive_types::{H160, U256};
use shared::conversions::U256Ext as _;
use std::collections::HashMap;

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Trade {
    pub order: Order,
    pub sell_token_index: usize,
    pub executed_amount: U256,
    pub scaled_unsubsidized_fee: U256,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct OrderTrade {
    pub trade: Trade,
    pub buy_token_index: usize,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct LiquidityOrderTrade {
    pub trade: Trade,
    pub buy_token_offset_index: usize,
    pub buy_token_price: U256,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct TradeExecution {
    pub sell_token: H160,
    pub buy_token: H160,
    pub sell_amount: U256,
    pub buy_amount: U256,
    pub fee_amount: U256,
}

impl Trade {
    // The difference between the minimum you were willing to buy/maximum you were willing to sell, and what you ended up buying/selling
    pub fn surplus(
        &self,
        sell_token_price: &BigRational,
        buy_token_price: &BigRational,
    ) -> Option<BigRational> {
        match self.order.creation.kind {
            model::order::OrderKind::Buy => buy_order_surplus(
                sell_token_price,
                buy_token_price,
                &self.order.creation.sell_amount.to_big_rational(),
                &self.order.creation.buy_amount.to_big_rational(),
                &self.executed_amount.to_big_rational(),
            ),
            model::order::OrderKind::Sell => sell_order_surplus(
                sell_token_price,
                buy_token_price,
                &self.order.creation.sell_amount.to_big_rational(),
                &self.order.creation.buy_amount.to_big_rational(),
                &self.executed_amount.to_big_rational(),
            ),
        }
    }

    pub fn surplus_ratio(
        &self,
        sell_token_price: &BigRational,
        buy_token_price: &BigRational,
    ) -> Option<BigRational> {
        surplus_ratio(
            sell_token_price,
            buy_token_price,
            &self.order.creation.sell_amount.to_big_rational(),
            &self.order.creation.buy_amount.to_big_rational(),
        )
    }

    // Returns the executed fee amount (prorated of executed amount)
    // cf. https://github.com/gnosis/gp-v2-contracts/blob/964f1eb76f366f652db7f4c2cb5ff9bfa26eb2cd/src/contracts/GPv2Settlement.sol#L370-L371
    pub fn executed_fee(&self) -> Option<U256> {
        self.compute_fee_execution(self.order.creation.fee_amount)
    }

    /// Returns the scaled unsubsidized fee amount that should be used for
    /// objective value computation.
    pub fn executed_scaled_unsubsidized_fee(&self) -> Option<U256> {
        self.compute_fee_execution(self.scaled_unsubsidized_fee)
    }

    pub fn executed_unscaled_subsidized_fee(&self) -> Option<U256> {
        self.compute_fee_execution(self.order.creation.fee_amount)
    }

    fn compute_fee_execution(&self, fee_amount: U256) -> Option<U256> {
        match self.order.creation.kind {
            model::order::OrderKind::Buy => fee_amount
                .checked_mul(self.executed_amount)?
                .checked_div(self.order.creation.buy_amount),
            model::order::OrderKind::Sell => fee_amount
                .checked_mul(self.executed_amount)?
                .checked_div(self.order.creation.sell_amount),
        }
    }

    /// Computes and returns the executed trade amounts given sell and buy prices.
    pub fn executed_amounts(&self, sell_price: U256, buy_price: U256) -> Option<TradeExecution> {
        let order = &self.order.creation;
        let (sell_amount, buy_amount) = match order.kind {
            OrderKind::Sell => {
                let sell_amount = self.executed_amount;
                let buy_amount = sell_amount
                    .checked_mul(sell_price)?
                    .checked_ceil_div(&buy_price)?;
                (sell_amount, buy_amount)
            }
            OrderKind::Buy => {
                let buy_amount = self.executed_amount;
                let sell_amount = buy_amount.checked_mul(buy_price)?.checked_div(sell_price)?;
                (sell_amount, buy_amount)
            }
        };
        let fee_amount = self.executed_fee()?;

        Some(TradeExecution {
            sell_token: order.sell_token,
            buy_token: order.buy_token,
            sell_amount,
            buy_amount,
            fee_amount,
        })
    }
}

impl OrderTrade {
    /// Encodes the settlement's order_trade as a tuple, as expected by the smart
    /// contract.
    pub fn encode(&self) -> EncodedTrade {
        encoding::encode_trade(
            &self.trade.order.creation,
            self.trade.sell_token_index,
            self.buy_token_index,
            &self.trade.executed_amount,
        )
    }
}

impl LiquidityOrderTrade {
    /// Encodes the settlement's liquidity_order_trade as a tuple, as expected by the smart
    /// contract.
    pub fn encode(&self, clearing_price_vec_length: usize) -> EncodedTrade {
        let buy_token_index = clearing_price_vec_length + self.buy_token_offset_index;
        encoding::encode_trade(
            &self.trade.order.creation,
            self.trade.sell_token_index,
            buy_token_index,
            &self.trade.executed_amount,
        )
    }
}

pub trait Interaction: std::fmt::Debug + Send + Sync {
    // TODO: not sure if this should return a result.
    // Write::write returns a result but we know we write to a vector in memory so we know it will
    // never fail. Then the question becomes whether interactions should be allowed to fail encoding
    // for other reasons.
    fn encode(&self) -> Vec<EncodedInteraction>;
}

#[cfg(test)]
impl Interaction for EncodedInteraction {
    fn encode(&self) -> Vec<EncodedInteraction> {
        vec![self.clone()]
    }
}

#[cfg(test)]
#[derive(Debug)]
pub struct NoopInteraction;

#[cfg(test)]
impl Interaction for NoopInteraction {
    fn encode(&self) -> Vec<EncodedInteraction> {
        Vec::new()
    }
}

#[derive(Debug, Clone, Default)]
pub struct Settlement {
    pub encoder: SettlementEncoder,
}

pub enum Revertable {
    NoRisk,
    HighRisk,
}

impl Settlement {
    /// Creates a new settlement builder for the specified clearing prices.
    pub fn new(clearing_prices: HashMap<H160, U256>) -> Self {
        Self {
            encoder: SettlementEncoder::new(clearing_prices),
        }
    }

    /// .
    pub fn with_liquidity<L>(&mut self, liquidity: &L, execution: L::Execution) -> Result<()>
    where
        L: Settleable,
    {
        liquidity
            .settlement_handling()
            .encode(execution, &mut self.encoder)
    }

    pub fn without_onchain_liquidity(&self) -> Self {
        let encoder = self.encoder.without_onchain_liquidity();
        Self { encoder }
    }

    #[cfg(test)]
    pub fn with_trades(
        clearing_prices: HashMap<H160, U256>,
        trades: Vec<OrderTrade>,
        liquidity_order_trades: Vec<LiquidityOrderTrade>,
    ) -> Self {
        let encoder =
            SettlementEncoder::with_trades(clearing_prices, trades, liquidity_order_trades);
        Self { encoder }
    }

    /// Returns the clearing prices map.
    pub fn clearing_prices(&self) -> &HashMap<H160, U256> {
        self.encoder.clearing_prices()
    }

    /// Returns the clearing price for the specified token.
    ///
    /// Returns `None` if the token is not part of the settlement.
    pub fn clearing_price(&self, token: H160) -> Option<U256> {
        self.clearing_prices().get(&token).copied()
    }

    /// Returns all orders included in the settlement.
    pub fn traded_orders(&self) -> impl Iterator<Item = &Order> + '_ {
        let user_orders = self
            .encoder
            .order_trades()
            .iter()
            .map(|trade| &trade.trade.order);
        let liquidity_orders = self
            .encoder
            .liquidity_order_trades()
            .iter()
            .map(|trade| &trade.trade.order);
        user_orders.chain(liquidity_orders)
    }

    /// Returns an iterator of all executed trades.
    pub fn executed_trades(&self) -> impl Iterator<Item = TradeExecution> + '_ {
        let order_trades = self.encoder.order_trades().iter().map(move |order_trade| {
            let order = &order_trade.trade.order.creation;
            order_trade.trade.executed_amounts(
                self.clearing_price(order.sell_token)?,
                self.clearing_price(order.buy_token)?,
            )
        });
        let liquidity_order_trades =
            self.encoder
                .liquidity_order_trades()
                .iter()
                .map(move |liquidity_order_trade| {
                    let order = &liquidity_order_trade.trade.order.creation;
                    liquidity_order_trade.trade.executed_amounts(
                        self.clearing_price(order.sell_token)?,
                        liquidity_order_trade.buy_token_price,
                    )
                });

        order_trades
            .chain(liquidity_order_trades)
            .map(|execution| execution.expect("invalid trade was added to encoder"))
    }

    // Computes the total surplus of all protocol trades (in wei ETH).
    pub fn total_surplus(&self, external_prices: &ExternalPrices) -> BigRational {
        match self.encoder.total_surplus(external_prices) {
            Some(value) => value,
            None => {
                tracing::error!("Overflow computing objective value for: {:?}", self);
                num::zero()
            }
        }
    }

    // Computes the total scaled unsubsidized fee of all protocol trades (in wei ETH).
    pub fn total_scaled_unsubsidized_fees(&self, external_prices: &ExternalPrices) -> BigRational {
        self.encoder
            .order_trades()
            .iter()
            .filter_map(|order_trade| {
                external_prices.try_get_native_amount(
                    order_trade.trade.order.creation.sell_token,
                    order_trade
                        .trade
                        .executed_scaled_unsubsidized_fee()?
                        .to_big_rational(),
                )
            })
            .sum()
    }

    // Computes the total scaled unsubsidized fee of all protocol trades (in wei ETH).
    pub fn total_unscaled_subsidized_fees(&self, external_prices: &ExternalPrices) -> BigRational {
        self.encoder
            .order_trades()
            .iter()
            .filter_map(|order_trade| {
                external_prices.try_get_native_amount(
                    order_trade.trade.order.creation.sell_token,
                    order_trade
                        .trade
                        .executed_unscaled_subsidized_fee()?
                        .to_big_rational(),
                )
            })
            .sum()
    }

    /// See SettlementEncoder::merge
    pub fn merge(self, other: Self) -> Result<Self> {
        let merged = self.encoder.merge(other.encoder)?;
        Ok(Self { encoder: merged })
    }

    // Calculates the risk level for settlement to be reverted
    pub fn revertable(&self) -> Revertable {
        if self.encoder.execution_plan().is_empty() {
            return Revertable::NoRisk;
        }
        Revertable::HighRisk
    }
}

impl From<Settlement> for EncodedSettlement {
    fn from(settlement: Settlement) -> Self {
        settlement.encoder.finish()
    }
}

// The difference between what you were willing to sell (executed_amount * limit_price)
// converted into reference token (multiplied by buy_token_price)
// and what you had to sell denominated in the reference token (executed_amount * buy_token_price)
fn buy_order_surplus(
    sell_token_price: &BigRational,
    buy_token_price: &BigRational,
    sell_amount_limit: &BigRational,
    buy_amount_limit: &BigRational,
    executed_buy_amount: &BigRational,
) -> Option<BigRational> {
    if buy_amount_limit.is_zero() {
        return None;
    }
    let limit_sell_amount = executed_buy_amount * sell_amount_limit / buy_amount_limit;
    let res = (limit_sell_amount * sell_token_price) - (executed_buy_amount * buy_token_price);
    if res.is_negative() {
        None
    } else {
        Some(res)
    }
}

// The difference of your proceeds denominated in the reference token (executed_sell_amount * sell_token_price)
// and what you were minimally willing to receive in buy tokens (executed_sell_amount * limit_price)
// converted to amount in reference token at the effective price (multiplied by buy_token_price)
fn sell_order_surplus(
    sell_token_price: &BigRational,
    buy_token_price: &BigRational,
    sell_amount_limit: &BigRational,
    buy_amount_limit: &BigRational,
    executed_sell_amount: &BigRational,
) -> Option<BigRational> {
    if sell_amount_limit.is_zero() {
        return None;
    }
    let limit_buy_amount = executed_sell_amount * buy_amount_limit / sell_amount_limit;
    let res = (executed_sell_amount * sell_token_price) - (limit_buy_amount * buy_token_price);
    if res.is_negative() {
        None
    } else {
        Some(res)
    }
}

/// Surplus Ratio represents the percentage difference of the executed price with the limit price.
/// This is calculated for orders with a corresponding trade. This value is always non-negative
/// since orders are contractually bound to be settled on or beyond their limit price.
fn surplus_ratio(
    sell_token_price: &BigRational,
    buy_token_price: &BigRational,
    sell_amount_limit: &BigRational,
    buy_amount_limit: &BigRational,
) -> Option<BigRational> {
    if buy_token_price.is_zero() || buy_amount_limit.is_zero() {
        return None;
    }
    // We subtract 1 here to give the give the percent beyond limit price instead of the
    // whole amount according to the definition of "surplus" (that which is more).
    let res = (sell_amount_limit * sell_token_price) / (buy_amount_limit * buy_token_price)
        - BigRational::one();
    if res.is_negative() {
        return None;
    }
    Some(res)
}

#[cfg(test)]
pub mod tests {
    use super::*;
    use crate::{liquidity::SettlementHandling, settlement::external_prices::externalprices};
    use maplit::hashmap;
    use model::order::{OrderCreation, OrderKind};
    use num::FromPrimitive;
    use shared::addr;

    pub fn assert_settlement_encoded_with<L, S>(
        prices: HashMap<H160, U256>,
        handler: S,
        execution: L::Execution,
        exec: impl FnOnce(&mut SettlementEncoder),
    ) where
        L: Settleable,
        S: SettlementHandling<L>,
    {
        let actual_settlement = {
            let mut encoder = SettlementEncoder::new(prices.clone());
            handler.encode(execution, &mut encoder).unwrap();
            encoder.finish()
        };
        let expected_settlement = {
            let mut encoder = SettlementEncoder::new(prices);
            exec(&mut encoder);
            encoder.finish()
        };

        assert_eq!(actual_settlement, expected_settlement);
    }

    // Helper function to save some repetition below.
    fn r(u: u128) -> BigRational {
        BigRational::from_u128(u).unwrap()
    }

    /// Helper function for creating a settlement for the specified prices and
    /// trades for testing objective value computations.
    fn test_settlement(
        prices: HashMap<H160, U256>,
        trades: Vec<OrderTrade>,
        liquidity_order_trades: Vec<LiquidityOrderTrade>,
    ) -> Settlement {
        Settlement {
            encoder: SettlementEncoder::with_trades(prices, trades, liquidity_order_trades),
        }
    }

    #[test]
    fn sell_order_executed_amounts() {
        let trade = Trade {
            order: Order {
                creation: OrderCreation {
                    kind: OrderKind::Sell,
                    sell_amount: 10.into(),
                    buy_amount: 6.into(),
                    ..Default::default()
                },
                ..Default::default()
            },
            executed_amount: 5.into(),
            ..Default::default()
        };
        let sell_price = 3.into();
        let buy_price = 4.into();
        let execution = trade.executed_amounts(sell_price, buy_price).unwrap();

        assert_eq!(execution.sell_amount, 5.into());
        assert_eq!(execution.buy_amount, 4.into()); // round up!
    }

    #[test]
    fn buy_order_executed_amounts() {
        let trade = Trade {
            order: Order {
                creation: OrderCreation {
                    kind: OrderKind::Buy,
                    sell_amount: 10.into(),
                    buy_amount: 6.into(),
                    ..Default::default()
                },
                ..Default::default()
            },
            executed_amount: 5.into(),
            ..Default::default()
        };
        let sell_price = 3.into();
        let buy_price = 4.into();
        let execution = trade.executed_amounts(sell_price, buy_price).unwrap();

        assert_eq!(execution.sell_amount, 6.into()); // round down!
        assert_eq!(execution.buy_amount, 5.into());
    }

    #[test]
    fn trade_order_executed_amounts_overflow() {
        for kind in [OrderKind::Sell, OrderKind::Buy] {
            let order = Order {
                creation: OrderCreation {
                    kind,
                    ..Default::default()
                },
                ..Default::default()
            };

            // mul
            let trade = Trade {
                order: order.clone(),
                executed_amount: U256::MAX,
                ..Default::default()
            };
            let sell_price = U256::from(2);
            let buy_price = U256::one();
            assert!(trade.executed_amounts(sell_price, buy_price).is_none());

            // div
            let trade = Trade {
                order,
                executed_amount: U256::one(),
                ..Default::default()
            };
            let sell_price = U256::one();
            let buy_price = U256::zero();
            assert!(trade.executed_amounts(sell_price, buy_price).is_none());
        }
    }

    #[test]
    fn total_surplus() {
        let token0 = H160::from_low_u64_be(0);
        let token1 = H160::from_low_u64_be(1);

        let order0 = Order {
            creation: OrderCreation {
                sell_token: token0,
                buy_token: token1,
                sell_amount: 10.into(),
                buy_amount: 9.into(),
                kind: OrderKind::Sell,
                ..Default::default()
            },
            ..Default::default()
        };
        let order1 = Order {
            creation: OrderCreation {
                sell_token: token1,
                buy_token: token0,
                sell_amount: 10.into(),
                buy_amount: 9.into(),
                kind: OrderKind::Sell,
                ..Default::default()
            },
            ..Default::default()
        };

        let trade0 = OrderTrade {
            trade: Trade {
                order: order0.clone(),
                executed_amount: 10.into(),
                ..Default::default()
            },
            ..Default::default()
        };
        let trade1 = OrderTrade {
            trade: Trade {
                order: order1.clone(),
                executed_amount: 10.into(),
                ..Default::default()
            },
            ..Default::default()
        };

        // Case where external price vector doesn't influence ranking:

        let clearing_prices0 = hashmap! {token0 => 1.into(), token1 => 1.into()};
        let clearing_prices1 = hashmap! {token0 => 2.into(), token1 => 2.into()};

        let settlement0 = test_settlement(
            clearing_prices0,
            vec![trade0.clone(), trade1.clone()],
            vec![],
        );

        let settlement1 = test_settlement(clearing_prices1, vec![trade0, trade1], vec![]);

        let external_prices = externalprices! { native_token: token0, token1 => r(1) };
        assert_eq!(
            settlement0.total_surplus(&external_prices),
            settlement1.total_surplus(&external_prices)
        );

        let external_prices = externalprices! { native_token: token0, token1 => r(1)/r(2) };
        assert_eq!(
            settlement0.total_surplus(&external_prices),
            settlement1.total_surplus(&external_prices)
        );

        // Case where external price vector influences ranking:

        let trade0 = OrderTrade {
            trade: Trade {
                order: order0.clone(),
                executed_amount: 10.into(),
                ..Default::default()
            },
            ..Default::default()
        };
        let trade1 = OrderTrade {
            trade: Trade {
                order: order1.clone(),
                executed_amount: 9.into(),
                ..Default::default()
            },
            ..Default::default()
        };

        let clearing_prices0 = hashmap! {token0 => 9.into(), token1 => 10.into()};

        // Settlement0 gets the following surpluses:
        // trade0: 81 - 81 = 0
        // trade1: 100 - 81 = 19
        let settlement0 = test_settlement(clearing_prices0, vec![trade0, trade1], vec![]);

        let trade0 = OrderTrade {
            trade: Trade {
                order: order0,
                executed_amount: 9.into(),
                ..Default::default()
            },
            ..Default::default()
        };
        let trade1 = OrderTrade {
            trade: Trade {
                order: order1,
                executed_amount: 10.into(),
                ..Default::default()
            },
            ..Default::default()
        };

        let clearing_prices1 = hashmap! {token0 => 10.into(), token1 => 9.into()};

        // Settlement1 gets the following surpluses:
        // trade0: 90 - 72.9 = 17.1
        // trade1: 100 - 100 = 0
        let settlement1 = test_settlement(clearing_prices1, vec![trade0, trade1], vec![]);

        // If the external prices of the two tokens is the same, then both settlements are symmetric.
        let external_prices = externalprices! { native_token: token0, token1 => r(1) };
        assert_eq!(
            settlement0.total_surplus(&external_prices),
            settlement1.total_surplus(&external_prices)
        );

        // If the external price of the first token is higher, then the first settlement is preferred.
        let external_prices = externalprices! { native_token: token0, token1 => r(1)/r(2) };

        // Settlement0 gets the following normalized surpluses:
        // trade0: 0
        // trade1: 19 * 2 / 10 = 3.8

        // Settlement1 gets the following normalized surpluses:
        // trade0: 17.1 * 1 / 9 = 1.9
        // trade1: 0

        assert!(
            settlement0.total_surplus(&external_prices)
                > settlement1.total_surplus(&external_prices)
        );

        // If the external price of the second token is higher, then the second settlement is preferred.
        // (swaps above normalized surpluses of settlement0 and settlement1)
        let external_prices = externalprices! { native_token: token0, token1 => r(2) };
        assert!(
            settlement0.total_surplus(&external_prices)
                < settlement1.total_surplus(&external_prices)
        );
    }

    #[test]
    fn test_computing_objective_value_with_zero_prices() {
        // Test if passing a clearing price of zero to the objective value function does
        // not panic.

        let token0 = H160::from_low_u64_be(0);
        let token1 = H160::from_low_u64_be(1);

        let order = Order {
            creation: OrderCreation {
                sell_token: token0,
                buy_token: token1,
                sell_amount: 10.into(),
                buy_amount: 9.into(),
                kind: OrderKind::Sell,
                ..Default::default()
            },
            ..Default::default()
        };

        let trade = OrderTrade {
            trade: Trade {
                order,
                executed_amount: 10.into(),
                ..Default::default()
            },
            ..Default::default()
        };

        let clearing_prices = hashmap! {token0 => 1.into(), token1 => 0.into()};

        let settlement = test_settlement(clearing_prices, vec![trade], vec![]);

        let external_prices = externalprices! { native_token: token0, token1 => r(1) };
        settlement.total_surplus(&external_prices);
    }

    #[test]
    #[allow(clippy::just_underscores_and_digits)]
    fn test_buy_order_surplus() {
        // Two goods are worth the same (100 each). If we were willing to pay up to 60 to receive 50,
        // but ended paying the price (1) we have a surplus of 10 sell units, so a total surplus of 1000.
        assert_eq!(
            buy_order_surplus(&r(100), &r(100), &r(60), &r(50), &r(50)),
            Some(r(1000))
        );

        // If our trade got only half filled, we only get half the surplus
        assert_eq!(
            buy_order_surplus(&r(100), &r(100), &r(60), &r(50), &r(25)),
            Some(r(500))
        );

        // No surplus if trade is not at all filled
        assert_eq!(
            buy_order_surplus(&r(100), &r(100), &r(60), &r(50), &r(0)),
            Some(r(0))
        );

        // No surplus if trade is filled at limit
        assert_eq!(
            buy_order_surplus(&r(100), &r(100), &r(50), &r(50), &r(50)),
            Some(r(0))
        );

        // Arithmetic error when limit price not respected
        assert_eq!(
            buy_order_surplus(&r(100), &r(100), &r(40), &r(50), &r(50)),
            None
        );

        // Sell Token worth twice as much as buy token. If we were willing to sell at parity, we will
        // have a surplus of 50% of tokens, worth 200 each.
        assert_eq!(
            buy_order_surplus(&r(200), &r(100), &r(50), &r(50), &r(50)),
            Some(r(5000))
        );

        // Buy Token worth twice as much as sell token. If we were willing to sell at 3:1, we will
        // have a surplus of 20 sell tokens, worth 100 each.
        assert_eq!(
            buy_order_surplus(&r(100), &r(200), &r(60), &r(20), &r(20)),
            Some(r(2000))
        );
    }

    #[test]
    #[allow(clippy::just_underscores_and_digits)]
    fn test_sell_order_surplus() {
        // Two goods are worth the same (100 each). If we were willing to receive as little as 40,
        // but ended paying the price (1) we have a surplus of 10 bought units, so a total surplus of 1000.
        assert_eq!(
            sell_order_surplus(&r(100), &r(100), &r(50), &r(40), &r(50)),
            Some(r(1000))
        );

        // If our trade got only half filled, we only get half the surplus
        assert_eq!(
            sell_order_surplus(&r(100), &r(100), &r(50), &r(40), &r(25)),
            Some(r(500))
        );

        // No surplus if trade is not at all filled
        assert_eq!(
            sell_order_surplus(&r(100), &r(100), &r(50), &r(40), &r(0)),
            Some(r(0))
        );

        // No surplus if trade is filled at limit
        assert_eq!(
            sell_order_surplus(&r(100), &r(100), &r(50), &r(50), &r(50)),
            Some(r(0))
        );

        // Arithmetic error when limit price not respected
        assert_eq!(
            sell_order_surplus(&r(100), &r(100), &r(50), &r(60), &r(50)),
            None
        );

        // Sell token worth twice as much as buy token. If we were willing to buy at parity, we will
        // have a surplus of 100% of buy tokens, worth 100 each.
        assert_eq!(
            sell_order_surplus(&r(200), &r(100), &r(50), &r(50), &r(50)),
            Some(r(5000))
        );

        // Buy Token worth twice as much as sell token. If we were willing to sell at 3:1, we will
        // have a surplus of 10 buy tokens, worth 200 each.
        assert_eq!(
            sell_order_surplus(&r(100), &r(200), &r(60), &r(20), &r(60)),
            Some(r(2000))
        );
    }

    #[test]
    #[allow(clippy::just_underscores_and_digits)]
    fn test_surplus_ratio() {
        assert_eq!(
            surplus_ratio(&r(1), &r(1), &r(1), &r(1)),
            Some(BigRational::zero())
        );
        assert_eq!(
            surplus_ratio(&r(1), &BigRational::new(1.into(), 2.into()), &r(1), &r(1)),
            Some(r(1))
        );

        assert_eq!(surplus_ratio(&r(2), &r(1), &r(1), &r(1)), Some(r(1)));

        // Two goods are worth the same (100 each). If we were willing to sell up to 60 to receive 50,
        // but ended paying the price (1) we have a surplus of 10 sell units, so a total surplus of 1000.
        assert_eq!(
            surplus_ratio(&r(100), &r(100), &r(60), &r(50)),
            Some(BigRational::new(1.into(), 5.into())),
        );

        // No surplus if trade is filled at limit
        assert_eq!(surplus_ratio(&r(100), &r(100), &r(50), &r(50)), Some(r(0)));

        // Arithmetic error when limit price not respected
        assert_eq!(surplus_ratio(&r(100), &r(100), &r(40), &r(50)), None);

        // Sell Token worth twice as much as buy token. If we were willing to sell at parity, we will
        // have a surplus of 50% of tokens, worth 200 each.
        assert_eq!(surplus_ratio(&r(200), &r(100), &r(50), &r(50)), Some(r(1)));

        // Buy Token worth twice as much as sell token. If we were willing to sell at 3:1, we will
        // have a surplus of 20 sell tokens, worth 100 each.
        assert_eq!(
            surplus_ratio(&r(100), &r(200), &r(60), &r(20)),
            Some(BigRational::new(1.into(), 2.into()))
        );
    }

    #[test]
    fn test_trade_fee() {
        let fully_filled_sell = Trade {
            order: Order {
                creation: OrderCreation {
                    sell_amount: 100.into(),
                    fee_amount: 5.into(),
                    kind: OrderKind::Sell,
                    ..Default::default()
                },
                ..Default::default()
            },
            executed_amount: 100.into(),
            ..Default::default()
        };
        assert_eq!(fully_filled_sell.executed_fee().unwrap(), 5.into());

        let partially_filled_sell = Trade {
            order: Order {
                creation: OrderCreation {
                    sell_amount: 100.into(),
                    fee_amount: 5.into(),
                    kind: OrderKind::Sell,
                    ..Default::default()
                },
                ..Default::default()
            },
            executed_amount: 50.into(),
            ..Default::default()
        };
        assert_eq!(partially_filled_sell.executed_fee().unwrap(), 2.into());

        let fully_filled_buy = Trade {
            order: Order {
                creation: OrderCreation {
                    buy_amount: 100.into(),
                    fee_amount: 5.into(),
                    kind: OrderKind::Buy,
                    ..Default::default()
                },
                ..Default::default()
            },
            executed_amount: 100.into(),
            ..Default::default()
        };
        assert_eq!(fully_filled_buy.executed_fee().unwrap(), 5.into());

        let partially_filled_buy = Trade {
            order: Order {
                creation: OrderCreation {
                    buy_amount: 100.into(),
                    fee_amount: 5.into(),
                    kind: OrderKind::Buy,
                    ..Default::default()
                },
                ..Default::default()
            },
            executed_amount: 50.into(),
            ..Default::default()
        };
        assert_eq!(partially_filled_buy.executed_fee().unwrap(), 2.into());
    }

    #[test]
    fn test_trade_fee_overflow() {
        let large_amounts = Trade {
            order: Order {
                creation: OrderCreation {
                    sell_amount: U256::max_value(),
                    fee_amount: U256::max_value(),
                    kind: OrderKind::Sell,
                    ..Default::default()
                },
                ..Default::default()
            },
            executed_amount: U256::max_value(),
            ..Default::default()
        };
        assert_eq!(large_amounts.executed_fee(), None);

        let zero_amounts = Trade {
            order: Order {
                creation: OrderCreation {
                    sell_amount: U256::zero(),
                    fee_amount: U256::zero(),
                    kind: OrderKind::Sell,
                    ..Default::default()
                },
                ..Default::default()
            },
            executed_amount: U256::zero(),
            ..Default::default()
        };
        assert_eq!(zero_amounts.executed_fee(), None);
    }

    #[test]
    fn total_fees_normalizes_individual_fees_into_eth() {
        let token0 = H160::from_low_u64_be(0);
        let token1 = H160::from_low_u64_be(1);

        let trade0 = OrderTrade {
            trade: Trade {
                order: Order {
                    creation: OrderCreation {
                        sell_token: token0,
                        sell_amount: 10.into(),
                        fee_amount: 1.into(),
                        kind: OrderKind::Sell,
                        ..Default::default()
                    },
                    ..Default::default()
                },
                executed_amount: 10.into(),
                // Note that the scaled fee amount is different than the order's
                // signed fee amount. This happens for subsidized orders, and when
                // a fee objective scaling factor is configured.
                scaled_unsubsidized_fee: 5.into(),
                ..Default::default()
            },
            ..Default::default()
        };
        let trade1 = OrderTrade {
            trade: Trade {
                order: Order {
                    creation: OrderCreation {
                        sell_token: token1,
                        sell_amount: 10.into(),
                        fee_amount: 2.into(),
                        kind: OrderKind::Sell,
                        ..Default::default()
                    },
                    ..Default::default()
                },
                executed_amount: 10.into(),
                scaled_unsubsidized_fee: 2.into(),
                ..Default::default()
            },
            ..Default::default()
        };

        let clearing_prices = hashmap! {token0 => 5.into(), token1 => 10.into()};
        let external_prices = externalprices! {
            native_token: H160([0xff; 20]),
            token0 => BigRational::from_integer(5.into()),
            token1 => BigRational::from_integer(10.into()),
        };

        // Fee in sell tokens
        assert_eq!(trade0.trade.executed_fee().unwrap(), 1.into());
        assert_eq!(
            trade0.trade.executed_scaled_unsubsidized_fee().unwrap(),
            5.into()
        );
        assert_eq!(trade1.trade.executed_fee().unwrap(), 2.into());
        assert_eq!(
            trade1.trade.executed_scaled_unsubsidized_fee().unwrap(),
            2.into()
        );

        // Fee in wei of ETH
        let settlement = test_settlement(clearing_prices, vec![trade0, trade1], vec![]);
        assert_eq!(
            settlement.total_scaled_unsubsidized_fees(&external_prices),
            BigRational::from_integer(45.into())
        );
    }

    #[test]
    fn fees_excluded_for_pmm_orders() {
        let token0 = H160([0; 20]);
        let token1 = H160([1; 20]);
        let settlement = test_settlement(
            hashmap! { token0 => 1.into(), token1 => 1.into() },
            vec![OrderTrade {
                trade: Trade {
                    order: Order {
                        creation: OrderCreation {
                            sell_token: token0,
                            buy_token: token1,
                            sell_amount: 1.into(),
                            kind: OrderKind::Sell,
                            // Note that this fee amount is NOT used!
                            fee_amount: 6.into(),
                            ..Default::default()
                        },
                        ..Default::default()
                    },
                    executed_amount: 1.into(),
                    // This is what matters for the objective value
                    scaled_unsubsidized_fee: 42.into(),
                    ..Default::default()
                },
                ..Default::default()
            }],
            vec![LiquidityOrderTrade {
                trade: Trade {
                    order: Order {
                        creation: OrderCreation {
                            sell_token: token1,
                            buy_token: token0,
                            buy_amount: 1.into(),
                            kind: OrderKind::Buy,
                            fee_amount: 28.into(),
                            ..Default::default()
                        },
                        ..Default::default()
                    },
                    executed_amount: 1.into(),
                    // Doesn't count because it is a "liquidity order"
                    scaled_unsubsidized_fee: 1337.into(),
                    ..Default::default()
                },
                ..Default::default()
            }],
        );

        assert_eq!(
            settlement.total_scaled_unsubsidized_fees(&externalprices! { native_token: token0 }),
            r(42),
        );
    }

    #[test]
    fn prefers_amm_with_better_price_over_pmm() {
        let amm = test_settlement(
            hashmap! {
                addr!("4e3fbd56cd56c3e72c1403e103b45db9da5b9d2b") => 99760667014_u128.into(),
                addr!("dac17f958d2ee523a2206206994597c13d831ec7") => 3813250751402140530019_u128.into(),
            },
            vec![OrderTrade {
                trade: Trade {
                    order: Order {
                        creation: OrderCreation {
                            sell_token: addr!("dac17f958d2ee523a2206206994597c13d831ec7"),
                            buy_token: addr!("4e3fbd56cd56c3e72c1403e103b45db9da5b9d2b"),
                            sell_amount: 99760667014_u128.into(),
                            buy_amount: 3805639472457226077863_u128.into(),
                            fee_amount: 239332986_u128.into(),
                            kind: OrderKind::Sell,
                            ..Default::default()
                        },
                        ..Default::default()
                    },
                    executed_amount: 99760667014_u128.into(),
                    scaled_unsubsidized_fee: 239332986_u128.into(),
                    ..Default::default()
                },
                ..Default::default()
            }],
            vec![],
        );

        let pmm = test_settlement(
            hashmap! {
                addr!("4e3fbd56cd56c3e72c1403e103b45db9da5b9d2b") => 6174583113007029_u128.into(),
                addr!("a0b86991c6218b36c1d19d4a2e9eb0ce3606eb48") => 235665799111775530988005794_u128.into(),
                addr!("dac17f958d2ee523a2206206994597c13d831ec7") => 235593507027683452564881428_u128.into(),
            },
            vec![OrderTrade {
                trade: Trade {
                    order: Order {
                        creation: OrderCreation {
                            sell_token: addr!("dac17f958d2ee523a2206206994597c13d831ec7"),
                            buy_token: addr!("4e3fbd56cd56c3e72c1403e103b45db9da5b9d2b"),
                            sell_amount: 99760667014_u128.into(),
                            buy_amount: 3805639472457226077863_u128.into(),
                            fee_amount: 239332986_u128.into(),
                            kind: OrderKind::Sell,
                            ..Default::default()
                        },
                        ..Default::default()
                    },
                    executed_amount: 99760667014_u128.into(),
                    scaled_unsubsidized_fee: 239332986_u128.into(),
                    ..Default::default()
                },
                ..Default::default()
            }],
            vec![LiquidityOrderTrade {
                trade: Trade {
                    order: Order {
                        creation: OrderCreation {
                            sell_token: addr!("a0b86991c6218b36c1d19d4a2e9eb0ce3606eb48"),
                            buy_token: addr!("dac17f958d2ee523a2206206994597c13d831ec7"),
                            sell_amount: 99730064753_u128.into(),
                            buy_amount: 99760667014_u128.into(),
                            fee_amount: 10650127_u128.into(),
                            kind: OrderKind::Buy,
                            ..Default::default()
                        },
                        ..Default::default()
                    },
                    executed_amount: 99760667014_u128.into(),
                    scaled_unsubsidized_fee: 77577144_u128.into(),
                    ..Default::default()
                },
                ..Default::default()
            }],
        );

        let external_prices = externalprices! {
            native_token: addr!("c02aaa39b223fe8d0a0e5c4f27ead9083c756cc2"),
            addr!("4e3fbd56cd56c3e72c1403e103b45db9da5b9d2b") =>
                BigRational::new(250000000000000000_u128.into(), 40551883611992959283_u128.into()),
            addr!("a0b86991c6218b36c1d19d4a2e9eb0ce3606eb48") =>
                BigRational::new(40000000000000000_u128.into(), 168777939_u128.into()),
            addr!("dac17f958d2ee523a2206206994597c13d831ec7") =>
                BigRational::new(250000000000000000_u128.into(), 1055980021_u128.into()),
        };
        let gas_price = 105386573044;
        let objective_value = |settlement: &Settlement, gas: u128| {
            settlement.total_surplus(&external_prices)
                + settlement.total_scaled_unsubsidized_fees(&external_prices)
                - r(gas * gas_price)
        };

        // Prefer the AMM that uses more gas because it offers a better price
        // to the user!
        assert!(objective_value(&amm, 657196) > objective_value(&pmm, 405053));
    }
}
