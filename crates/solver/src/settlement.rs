pub mod external_prices;
mod settlement_encoder;

use self::external_prices::ExternalPrices;
pub use self::settlement_encoder::{verify_executed_amount, PricedTrade, SettlementEncoder};
use crate::{
    encoding::{self, EncodedSettlement, EncodedTrade},
    liquidity::Settleable,
};
use anyhow::{ensure, Result};
use itertools::Itertools;
use model::order::{Order, OrderKind};
use num::{rational::Ratio, BigInt, BigRational, One, Signed, Zero};
use primitive_types::{H160, U256};
use shared::{
    conversions::U256Ext as _,
    http_solver::model::{InternalizationStrategy, SubmissionPreference},
};
use std::{
    collections::{HashMap, HashSet},
    ops::{Mul, Sub},
};

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Trade {
    pub order: Order,
    pub executed_amount: U256,
    pub scaled_unsubsidized_fee: U256,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct TradeExecution {
    pub sell_token: H160,
    pub buy_token: H160,
    pub sell_amount: U256,
    pub buy_amount: U256,
    pub fee_amount: U256,
}

fn trade_surplus(
    order: &Order,
    executed_amount: U256,
    sell_token_price: &BigRational,
    buy_token_price: &BigRational,
) -> Option<BigRational> {
    match order.data.kind {
        model::order::OrderKind::Buy => buy_order_surplus(
            sell_token_price,
            buy_token_price,
            &order.data.sell_amount.to_big_rational(),
            &order.data.buy_amount.to_big_rational(),
            &executed_amount.to_big_rational(),
        ),
        model::order::OrderKind::Sell => sell_order_surplus(
            sell_token_price,
            buy_token_price,
            &order.data.sell_amount.to_big_rational(),
            &order.data.buy_amount.to_big_rational(),
            &executed_amount.to_big_rational(),
        ),
    }
}

pub fn trade_surplus_in_native_token(
    order: &Order,
    executed_amount: U256,
    external_prices: &ExternalPrices,
    clearing_prices: &HashMap<H160, U256>,
) -> Option<BigRational> {
    let sell_token_price = *clearing_prices
        .get(&order.data.sell_token)
        .expect("Solution with trade but without price for sell token");
    let buy_token_price = *clearing_prices
        .get(&order.data.buy_token)
        .expect("Solution with trade but without price for buy token");

    trade_surplus_in_native_token_with_prices(
        order,
        executed_amount,
        external_prices,
        sell_token_price,
        buy_token_price,
    )
}

fn trade_surplus_in_native_token_with_prices(
    order: &Order,
    executed_amount: U256,
    external_prices: &ExternalPrices,
    sell_token_price: U256,
    buy_token_price: U256,
) -> Option<BigRational> {
    let sell_token_clearing_price = sell_token_price.to_big_rational();
    let buy_token_clearing_price = buy_token_price.to_big_rational();

    if match order.data.kind {
        OrderKind::Sell => &buy_token_clearing_price,
        OrderKind::Buy => &sell_token_clearing_price,
    }
    .is_zero()
    {
        return None;
    }

    let surplus = trade_surplus(
        order,
        executed_amount,
        &sell_token_clearing_price,
        &buy_token_clearing_price,
    )?;
    let normalized_surplus = match order.data.kind {
        OrderKind::Sell => external_prices
            .get_native_amount(order.data.buy_token, surplus / buy_token_clearing_price),
        OrderKind::Buy => external_prices
            .get_native_amount(order.data.sell_token, surplus / sell_token_clearing_price),
    };
    Some(normalized_surplus)
}

impl Trade {
    pub fn surplus_ratio(
        &self,
        sell_token_price: &BigRational,
        buy_token_price: &BigRational,
    ) -> Option<BigRational> {
        surplus_ratio(
            sell_token_price,
            buy_token_price,
            &self.order.data.sell_amount.to_big_rational(),
            &self.order.data.buy_amount.to_big_rational(),
        )
    }

    // Returns the executed fee amount (prorated of executed amount)
    // cf. https://github.com/cowprotocol/contracts/blob/v1.1.2/src/contracts/GPv2Settlement.sol#L383-L385
    pub fn executed_fee(&self) -> Option<U256> {
        self.compute_fee_execution(self.order.data.fee_amount)
    }

    /// Returns the scaled unsubsidized fee amount that should be used for
    /// objective value computation.
    pub fn executed_scaled_unsubsidized_fee(&self) -> Option<U256> {
        self.compute_fee_execution(self.scaled_unsubsidized_fee)
    }

    pub fn executed_unscaled_subsidized_fee(&self) -> Option<U256> {
        self.compute_fee_execution(self.order.data.fee_amount)
    }

    fn compute_fee_execution(&self, fee_amount: U256) -> Option<U256> {
        match self.order.data.kind {
            model::order::OrderKind::Buy => fee_amount
                .checked_mul(self.executed_amount)?
                .checked_div(self.order.data.buy_amount),
            model::order::OrderKind::Sell => fee_amount
                .checked_mul(self.executed_amount)?
                .checked_div(self.order.data.sell_amount),
        }
    }

    /// Computes and returns the executed trade amounts given sell and buy prices.
    pub fn executed_amounts(&self, sell_price: U256, buy_price: U256) -> Option<TradeExecution> {
        let order = &self.order.data;
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

impl Trade {
    /// Encodes the settlement's order_trade as a tuple, as expected by the smart
    /// contract.
    pub fn encode(&self, sell_token_index: usize, buy_token_index: usize) -> EncodedTrade {
        encoding::encode_trade(
            &self.order.data,
            &self.order.signature,
            self.order.metadata.owner,
            sell_token_index,
            buy_token_index,
            &self.executed_amount,
        )
    }
}

#[cfg(test)]
use shared::interaction::{EncodedInteraction, Interaction};
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
    pub submitter: SubmissionPreference,
}

#[derive(Debug, Eq, PartialEq)]
pub enum Revertable {
    NoRisk,
    HighRisk,
}

pub enum PriceCheckTokens {
    All,
    Tokens(HashSet<H160>),
}

impl From<Option<Vec<H160>>> for PriceCheckTokens {
    fn from(token_list: Option<Vec<H160>>) -> Self {
        if let Some(tokens) = token_list {
            PriceCheckTokens::Tokens(HashSet::from_iter(tokens.into_iter()))
        } else {
            PriceCheckTokens::All
        }
    }
}

impl Settlement {
    /// Creates a new settlement builder for the specified clearing prices.
    pub fn new(clearing_prices: HashMap<H160, U256>) -> Self {
        Self {
            encoder: SettlementEncoder::new(clearing_prices),
            ..Default::default()
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
        Self {
            encoder,
            submitter: self.submitter.clone(),
        }
    }

    #[cfg(test)]
    pub fn with_trades(clearing_prices: HashMap<H160, U256>, trades: Vec<Trade>) -> Self {
        let encoder = SettlementEncoder::with_trades(clearing_prices, trades);
        Self {
            encoder,
            ..Default::default()
        }
    }

    #[cfg(test)]
    pub fn with_default_prices(trades: Vec<Trade>) -> Self {
        let clearing_prices = trades
            .iter()
            .flat_map(|trade| [trade.order.data.sell_token, trade.order.data.buy_token])
            .map(|token| (token, U256::from(1_000_000_000_000_000_000_u128)))
            .collect();
        let encoder = SettlementEncoder::with_trades(clearing_prices, trades);
        Self {
            encoder,
            ..Default::default()
        }
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
        self.encoder.all_trades().map(|trade| &trade.data.order)
    }

    /// Returns an iterator of all executed trades.
    pub fn trade_executions(&self) -> impl Iterator<Item = TradeExecution> + '_ {
        self.encoder.all_trades().map(|trade| {
            trade
                .executed_amounts()
                .expect("invalid trade was added to encoder")
        })
    }

    /// Returns an iterator over all trades.
    pub fn trades(&self) -> impl Iterator<Item = &'_ Trade> + '_ {
        self.encoder.all_trades().map(|trade| trade.data)
    }

    /// Returns an iterator over all user trades.
    pub fn user_trades(&self) -> impl Iterator<Item = &'_ Trade> + '_ {
        self.encoder.user_trades().map(|trade| trade.data)
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

    // Checks whether the settlement prices do not deviate more than max_settlement_price_deviation from the auction prices on certain pairs.
    pub fn satisfies_price_checks(
        &self,
        solver_name: &str,
        external_prices: &ExternalPrices,
        max_settlement_price_deviation: &Ratio<BigInt>,
        tokens_to_satisfy_price_test: &PriceCheckTokens,
    ) -> bool {
        if matches!(tokens_to_satisfy_price_test, PriceCheckTokens::Tokens(token_list) if token_list.is_empty())
        {
            return true;
        }
        // The following check is quadratic in run-time, although a similar check with linear run-time would also be possible.
        // For the linear implementation, one would have to find a unique scaling factor that scales the external prices into
        // the settlement prices. Though, this scaling factor is not easy to define, if reference tokens like ETH are missing.
        // Since the checks would heavily depend on this scaling factor, and its
        // derivation is non-trivial, we decided to go for the implementation with quadratic run time. Settlements
        // will not have enough tokens, such that the run-time is important.
        self
            .clearing_prices()
            .iter()
            .combinations(2)
            .all(|clearing_price_vector_combination| {
                let (sell_token, sell_price) = clearing_price_vector_combination[0];
                let clearing_price_sell_token = sell_price.to_big_rational();
                let (buy_token, buy_price) = clearing_price_vector_combination[1];
                let clearing_price_buy_token = buy_price.to_big_rational();

                if matches!(tokens_to_satisfy_price_test, PriceCheckTokens::Tokens(token_list) if (!token_list.contains(sell_token)) || !token_list.contains(buy_token))
                {
                    return true;
                }
                let external_price_sell_token = match external_prices.price(sell_token) {
                    Some(price) => price,
                    None => return true,
                };
                let external_price_buy_token = match external_prices.price(buy_token) {
                    Some(price) => price,
                    None => return true,
                };
                // Condition to check: Deviation of clearing prices is bigger than max_settlement_price deviation
                //
                // |clearing_price_sell_token / clearing_price_buy_token - external_price_sell_token / external_price_buy_token)|
                // |----------------------------------------------------------------------------------------------------------|
                // |                     clearing_price_sell_token / clearing_price_buy_token                                 |
                // is bigger than:
                // max_settlement_price_deviation
                //
                // This is equal to: |clearing_price_sell_token * external_price_buy_token - external_price_sell_token * clearing_price_buy_token|>
                // max_settlement_price_deviation * clearing_price_buy_token * external_price_buy_token * clearing_price_sell_token

                let price_check_result = clearing_price_sell_token
                    .clone()
                    .mul(external_price_buy_token)
                    .sub(&external_price_sell_token.mul(&clearing_price_buy_token)).abs()
                    .lt(&max_settlement_price_deviation
                    .mul(&external_price_buy_token.mul(&clearing_price_sell_token)));
                if !price_check_result {
                    tracing::debug!(
                        token_pair =% format!("{:?}-{:?}", sell_token, buy_token),
                        %solver_name, settlement =? self,
                        "price violation",
                    );
                }
                price_check_result
            })
    }

    // Computes the total scaled unsubsidized fee of all protocol trades (in wei ETH).
    pub fn total_scaled_unsubsidized_fees(&self, external_prices: &ExternalPrices) -> BigRational {
        self.user_trades()
            .filter_map(|trade| {
                external_prices.try_get_native_amount(
                    trade.order.data.sell_token,
                    trade.executed_scaled_unsubsidized_fee()?.to_big_rational(),
                )
            })
            .sum()
    }

    // Computes the total scaled unsubsidized fee of all protocol trades (in wei ETH).
    pub fn total_unscaled_subsidized_fees(&self, external_prices: &ExternalPrices) -> BigRational {
        self.user_trades()
            .filter_map(|trade| {
                external_prices.try_get_native_amount(
                    trade.order.data.sell_token,
                    trade.executed_unscaled_subsidized_fee()?.to_big_rational(),
                )
            })
            .sum()
    }

    /// See SettlementEncoder::merge
    pub fn merge(self, other: Self) -> Result<Self> {
        ensure!(self.submitter == other.submitter, "different submitters");
        let merged = self.encoder.merge(other.encoder)?;
        Ok(Self {
            encoder: merged,
            submitter: self.submitter,
        })
    }

    // Calculates the risk level for settlement to be reverted
    pub fn revertable(&self) -> Revertable {
        if self.encoder.has_interactions() {
            Revertable::HighRisk
        } else {
            Revertable::NoRisk
        }
    }

    pub fn encode(self, internalization_strategy: InternalizationStrategy) -> EncodedSettlement {
        self.encoder.finish(internalization_strategy)
    }

    pub fn encode_uninternalized_if_different(self) -> Option<EncodedSettlement> {
        if self.encoder.contains_internalized_interactions() {
            Some(
                self.encoder
                    .finish(InternalizationStrategy::EncodeAllInteractions),
            )
        } else {
            None
        }
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
    use model::order::{LimitOrderClass, OrderClass, OrderData, OrderKind, OrderMetadata};
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
            encoder.finish(InternalizationStrategy::SkipInternalizableInteraction)
        };
        let expected_settlement = {
            let mut encoder = SettlementEncoder::new(prices);
            exec(&mut encoder);
            encoder.finish(InternalizationStrategy::SkipInternalizableInteraction)
        };

        assert_eq!(actual_settlement, expected_settlement);
    }

    // Helper function to save some repetition below.
    fn r(u: u128) -> BigRational {
        BigRational::from_u128(u).unwrap()
    }

    /// Helper function for creating a settlement for the specified prices and
    /// trades for testing objective value computations.
    fn test_settlement(prices: HashMap<H160, U256>, trades: Vec<Trade>) -> Settlement {
        Settlement {
            encoder: SettlementEncoder::with_trades(prices, trades),
            ..Default::default()
        }
    }

    #[test]
    pub fn satisfies_price_checks_sorts_out_invalid_prices() {
        let native_token = H160::from_low_u64_be(0);
        let token0 = H160::from_low_u64_be(1);
        let token1 = H160::from_low_u64_be(2);
        let token2 = H160::from_low_u64_be(3);
        let token3 = H160::from_low_u64_be(4);
        let max_price_deviation = Ratio::from_float(0.02f64).unwrap();
        let clearing_prices =
            hashmap! {token0 => 50i32.into(), token1 => 100i32.into(), token2 => 103i32.into()};
        let settlement = test_settlement(clearing_prices, vec![]);

        let external_prices = ExternalPrices::new(
            native_token,
            hashmap! {token0 => BigInt::from(50i32).into(), token1 => BigInt::from(100i32).into(), token2 => BigInt::from(100i32).into()},
        ).unwrap();
        // Tolerance exceed on token2
        assert!(!settlement.satisfies_price_checks(
            "test_solver",
            &external_prices,
            &max_price_deviation,
            &None.into()
        ));
        // No tolerance exceeded on token0 and token1
        assert!(settlement.satisfies_price_checks(
            "test_solver",
            &external_prices,
            &max_price_deviation,
            &Some(vec!(token0, token1)).into()
        ));
        // Tolerance exceeded on token2
        assert!(!settlement.satisfies_price_checks(
            "test_solver",
            &external_prices,
            &max_price_deviation,
            &Some(vec!(token1, token2)).into()
        ));

        let external_prices = ExternalPrices::new(
            native_token,
            hashmap! {token0 => BigInt::from(100i32).into(), token1 => BigInt::from(200i32).into(), token2 => BigInt::from(205i32).into()},
        ).unwrap();
        // No tolerance exceeded
        assert!(settlement.satisfies_price_checks(
            "test_solver",
            &external_prices,
            &max_price_deviation,
            &None.into()
        ));

        let external_prices = ExternalPrices::new(
            native_token,
            hashmap! {token0 => BigInt::from(200i32).into()},
        )
        .unwrap();
        // If only 1 token should be checked: trivially satisfies equation
        assert!(settlement.satisfies_price_checks(
            "test_solver",
            &external_prices,
            &max_price_deviation,
            &None.into()
        ));

        let external_prices = ExternalPrices::new(
            native_token,
            hashmap! {token0 => BigInt::from(200i32).into(), token1 => BigInt::from(300i32).into()},
        )
        .unwrap();
        // Can deal with missing token1, tolerance exceeded on token1
        assert!(!settlement.satisfies_price_checks(
            "test_solver",
            &external_prices,
            &max_price_deviation,
            &Some(vec!(token0, token1, token2)).into()
        ));

        let external_prices = ExternalPrices::new(
            native_token,
            hashmap! {token0 => BigInt::from(100i32).into(), token2 => BigInt::from(205i32).into()},
        )
        .unwrap();
        // Can deal with missing token1, tolerance not exceeded
        assert!(settlement.satisfies_price_checks(
            "test_solver",
            &external_prices,
            &max_price_deviation,
            &Some(vec!(token0, token1, token2)).into()
        ));

        let external_prices = ExternalPrices::new(
            native_token,
            hashmap! {token3 => BigInt::from(100000i32).into()},
        )
        .unwrap();
        // Token3 from external price is not in settlement, hence, it should accept any price
        assert!(settlement.satisfies_price_checks(
            "test_solver",
            &external_prices,
            &max_price_deviation,
            &Some(vec!(token0, token1, token2, token3)).into()
        ));
        // If no tokens are in the check_list settlements always satisfy the check
        assert!(settlement.satisfies_price_checks(
            "test_solver",
            &external_prices,
            &max_price_deviation,
            &Some(vec!()).into()
        ));
    }

    #[test]
    fn sell_order_executed_amounts() {
        let trade = Trade {
            order: Order {
                data: OrderData {
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
                data: OrderData {
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
                data: OrderData {
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
            data: OrderData {
                sell_token: token0,
                buy_token: token1,
                sell_amount: 10.into(),
                buy_amount: 9.into(),
                kind: OrderKind::Sell,
                partially_fillable: true,
                ..Default::default()
            },
            ..Default::default()
        };
        let order1 = Order {
            data: OrderData {
                sell_token: token1,
                buy_token: token0,
                sell_amount: 10.into(),
                buy_amount: 9.into(),
                kind: OrderKind::Sell,
                partially_fillable: true,
                ..Default::default()
            },
            ..Default::default()
        };

        let trade0 = Trade {
            order: order0.clone(),
            executed_amount: 10.into(),
            ..Default::default()
        };
        let trade1 = Trade {
            order: order1.clone(),
            executed_amount: 10.into(),
            ..Default::default()
        };

        // Case where external price vector doesn't influence ranking:

        let clearing_prices0 = hashmap! {token0 => 1.into(), token1 => 1.into()};
        let clearing_prices1 = hashmap! {token0 => 2.into(), token1 => 2.into()};

        let settlement0 = test_settlement(clearing_prices0, vec![trade0.clone(), trade1.clone()]);
        let settlement1 = test_settlement(clearing_prices1, vec![trade0, trade1]);

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

        let trade0 = Trade {
            order: order0.clone(),
            executed_amount: 10.into(),
            ..Default::default()
        };
        let trade1 = Trade {
            order: order1.clone(),
            executed_amount: 9.into(),
            ..Default::default()
        };

        let clearing_prices0 = hashmap! {token0 => 9.into(), token1 => 10.into()};

        // Settlement0 gets the following surpluses:
        // trade0: 81 - 81 = 0
        // trade1: 100 - 81 = 19
        let settlement0 = test_settlement(clearing_prices0, vec![trade0, trade1]);

        let trade0 = Trade {
            order: order0,
            executed_amount: 9.into(),
            ..Default::default()
        };
        let trade1 = Trade {
            order: order1,
            executed_amount: 10.into(),
            ..Default::default()
        };

        let clearing_prices1 = hashmap! {token0 => 10.into(), token1 => 9.into()};

        // Settlement1 gets the following surpluses:
        // trade0: 90 - 72.9 = 17.1
        // trade1: 100 - 100 = 0
        let settlement1 = test_settlement(clearing_prices1, vec![trade0, trade1]);

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
    fn test_constructing_settlement_with_zero_prices() {
        // Test if passing a clearing price of zero makes it not possible to add
        // trades.

        let token0 = H160::from_low_u64_be(0);
        let token1 = H160::from_low_u64_be(1);

        let order = Order {
            data: OrderData {
                sell_token: token0,
                buy_token: token1,
                sell_amount: 10.into(),
                buy_amount: 9.into(),
                kind: OrderKind::Sell,
                ..Default::default()
            },
            ..Default::default()
        };

        let mut settlement = Settlement::new(hashmap! {
            token0 => 1.into(),
            token1 => 1.into(),
        });
        assert!(settlement
            .encoder
            .add_trade(order.clone(), 10.into(), 0.into())
            .is_ok());

        let mut settlement = Settlement::new(hashmap! {
            token0 => 1.into(),
            token1 => 0.into(),
        });
        assert!(settlement
            .encoder
            .add_trade(order, 10.into(), 0.into())
            .is_err());
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
                data: OrderData {
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
                data: OrderData {
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
                data: OrderData {
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
                data: OrderData {
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
                data: OrderData {
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
                data: OrderData {
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

        let trade0 = Trade {
            order: Order {
                data: OrderData {
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
        };
        let trade1 = Trade {
            order: Order {
                data: OrderData {
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
        };

        let clearing_prices = hashmap! {token0 => 5.into(), token1 => 10.into()};
        let external_prices = externalprices! {
            native_token: H160([0xff; 20]),
            token0 => BigRational::from_integer(5.into()),
            token1 => BigRational::from_integer(10.into()),
        };

        // Fee in sell tokens
        assert_eq!(trade0.executed_fee().unwrap(), 1.into());
        assert_eq!(trade0.executed_scaled_unsubsidized_fee().unwrap(), 5.into());
        assert_eq!(trade1.executed_fee().unwrap(), 2.into());
        assert_eq!(trade1.executed_scaled_unsubsidized_fee().unwrap(), 2.into());

        // Fee in wei of ETH
        let settlement = test_settlement(clearing_prices, vec![trade0, trade1]);
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
            vec![
                Trade {
                    order: Order {
                        data: OrderData {
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
                },
                Trade {
                    order: Order {
                        data: OrderData {
                            sell_token: token1,
                            buy_token: token0,
                            buy_amount: 1.into(),
                            kind: OrderKind::Buy,
                            fee_amount: 28.into(),
                            ..Default::default()
                        },
                        metadata: OrderMetadata {
                            class: OrderClass::Liquidity,
                            ..Default::default()
                        },
                        ..Default::default()
                    },
                    executed_amount: 1.into(),
                    // Doesn't count because it is a "liquidity order"
                    scaled_unsubsidized_fee: 1337.into(),
                },
            ],
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
            vec![Trade {
                order: Order {
                    data: OrderData {
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
            }],
        );

        let pmm = test_settlement(
            hashmap! {
                addr!("4e3fbd56cd56c3e72c1403e103b45db9da5b9d2b") => 6174583113007029_u128.into(),
                addr!("a0b86991c6218b36c1d19d4a2e9eb0ce3606eb48") => 235665799111775530988005794_u128.into(),
                addr!("dac17f958d2ee523a2206206994597c13d831ec7") => 235593507027683452564881428_u128.into(),
            },
            vec![
                Trade {
                    order: Order {
                        data: OrderData {
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
                },
                Trade {
                    order: Order {
                        data: OrderData {
                            sell_token: addr!("a0b86991c6218b36c1d19d4a2e9eb0ce3606eb48"),
                            buy_token: addr!("dac17f958d2ee523a2206206994597c13d831ec7"),
                            sell_amount: 99730064753_u128.into(),
                            buy_amount: 99760667014_u128.into(),
                            fee_amount: 10650127_u128.into(),
                            kind: OrderKind::Buy,
                            ..Default::default()
                        },
                        metadata: OrderMetadata {
                            class: OrderClass::Liquidity,
                            ..Default::default()
                        },
                        ..Default::default()
                    },
                    executed_amount: 99760667014_u128.into(),
                    scaled_unsubsidized_fee: 77577144_u128.into(),
                },
            ],
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

    #[test]
    fn computes_limit_order_surplus_without_fees() {
        let native_token = H160([0xe; 20]);
        let tokens = [H160([1; 20]), H160([2; 20]), H160([3; 20])];

        let external_prices = externalprices! {
            native_token: native_token,
            tokens[0] => BigRational::one(),
            tokens[1] => BigRational::one(),
            tokens[2] => BigRational::one(),
        };

        for kind in [OrderKind::Sell, OrderKind::Buy] {
            // Settlement where there is surplus, but all of it is taken as
            // protocol fees - so total surplus should be 0.
            let no_surplus = test_settlement(
                hashmap! {
                    tokens[0] => 100_000_u128.into(),
                    tokens[1] => 100_000_u128.into(),
                },
                vec![Trade {
                    order: Order {
                        data: OrderData {
                            sell_token: tokens[0],
                            buy_token: tokens[1],
                            sell_amount: 100_000_u128.into(),
                            buy_amount: 99_000_u128.into(),
                            kind,
                            ..Default::default()
                        },
                        metadata: OrderMetadata {
                            class: OrderClass::Limit(LimitOrderClass {
                                surplus_fee: Some(1_000_u128.into()),
                                ..Default::default()
                            }),
                            ..Default::default()
                        },
                        ..Default::default()
                    },
                    executed_amount: 100_000_u128.into(),
                    scaled_unsubsidized_fee: 1_000_u128.into(),
                }],
            );

            assert_eq!(
                no_surplus.total_surplus(&external_prices).to_integer(),
                BigInt::zero(),
            );

            let some_surplus = test_settlement(
                hashmap! {
                    tokens[0] => 100_000_u128.into(),
                    tokens[2] => 100_000_u128.into(),
                },
                vec![Trade {
                    order: Order {
                        data: OrderData {
                            sell_token: tokens[0],
                            buy_token: tokens[2],
                            sell_amount: 100_000_u128.into(),
                            buy_amount: 98_000_u128.into(),
                            kind,
                            ..Default::default()
                        },
                        metadata: OrderMetadata {
                            class: OrderClass::Limit(LimitOrderClass {
                                surplus_fee: Some(1_000_u128.into()),
                                ..Default::default()
                            }),
                            ..Default::default()
                        },
                        ..Default::default()
                    },
                    executed_amount: 100_000_u128.into(),
                    scaled_unsubsidized_fee: 1_000_u128.into(),
                }],
            );

            assert_eq!(
                some_surplus.total_surplus(&external_prices).to_integer(),
                BigInt::from(1000_u128),
            );
        }
    }

    #[test]
    fn includes_limit_order_ucp() {
        let sell_token = H160([1; 20]);
        let buy_token = H160([2; 20]);

        let settlement = test_settlement(
            hashmap! {
                sell_token => 100_000_u128.into(),
                buy_token => 100_000_u128.into(),
            },
            vec![Trade {
                order: Order {
                    data: OrderData {
                        sell_token,
                        buy_token,
                        sell_amount: 100_000_u128.into(),
                        buy_amount: 99_000_u128.into(),
                        kind: OrderKind::Sell,
                        ..Default::default()
                    },
                    metadata: OrderMetadata {
                        class: OrderClass::Limit(LimitOrderClass {
                            surplus_fee: Some(1_000_u128.into()),
                            ..Default::default()
                        }),
                        ..Default::default()
                    },
                    ..Default::default()
                },
                executed_amount: 100_000_u128.into(),
                scaled_unsubsidized_fee: 1_000_u128.into(),
            }],
        )
        .encode(InternalizationStrategy::SkipInternalizableInteraction);

        // Note that for limit order **both** the uniform clearing price of the
        // buy token as well as the executed clearing price accounting for fees
        // are included.
        assert_eq!(
            settlement.tokens,
            [sell_token, buy_token, sell_token, buy_token]
        );
        assert_eq!(
            settlement.clearing_prices,
            [
                100_000_u128.into(),
                100_000_u128.into(),
                99_000_u128.into(),
                100_000_u128.into(),
            ],
        );
    }
}
