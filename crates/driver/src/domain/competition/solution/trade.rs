use {
    crate::domain::competition::{
        self,
        order::{self, FeePolicy, SellAmount, Side, TargetAmount, Uid},
        solution::error::{self, Math},
    },
    eth_domain_types::{self as eth, Asset},
    number::u256_ext::U256Ext,
};

/// A trade which executes an order as part of this solution.
#[derive(Debug, Clone)]
pub enum Trade {
    Fulfillment(Fulfillment),
    Jit(Jit),
}

impl Trade {
    pub fn uid(&self) -> Uid {
        match self {
            Trade::Fulfillment(fulfillment) => fulfillment.order().uid,
            Trade::Jit(jit) => jit.order().uid,
        }
    }

    pub fn side(&self) -> Side {
        match self {
            Trade::Fulfillment(fulfillment) => fulfillment.order().side,
            Trade::Jit(jit) => jit.order().side,
        }
    }

    pub fn protocol_fees(&self) -> Vec<FeePolicy> {
        match self {
            Trade::Fulfillment(fulfillment) => fulfillment.order().protocol_fees.to_vec(),
            Trade::Jit(_) => vec![],
        }
    }

    pub fn executed(&self) -> TargetAmount {
        match self {
            Trade::Fulfillment(fulfillment) => fulfillment.executed(),
            Trade::Jit(jit) => jit.executed(),
        }
    }

    pub fn fee(&self) -> SellAmount {
        match self {
            Trade::Fulfillment(fulfillment) => fulfillment.fee(),
            Trade::Jit(jit) => jit.fee,
        }
    }

    pub fn buy(&self) -> Asset {
        match self {
            Trade::Fulfillment(fulfillment) => fulfillment.order().buy,
            Trade::Jit(jit) => jit.order().buy,
        }
    }

    pub fn sell(&self) -> Asset {
        match self {
            Trade::Fulfillment(fulfillment) => fulfillment.order().sell,
            Trade::Jit(jit) => jit.order().sell,
        }
    }

    /// The effective amount that left the user's wallet including all fees.
    pub(crate) fn sell_amount(
        &self,
        prices: &ClearingPrices,
    ) -> Result<eth::TokenAmount, error::Math> {
        match self {
            Trade::Fulfillment(fulfillment) => fulfillment.sell_amount(prices),
            Trade::Jit(jit) => jit.sell_amount(prices),
        }
    }

    /// The effective amount the user received after all fees.
    ///
    /// Settlement contract uses `ceil` division for buy amount calculation.
    pub(crate) fn buy_amount(
        &self,
        prices: &ClearingPrices,
    ) -> Result<eth::TokenAmount, error::Math> {
        match self {
            Trade::Fulfillment(fulfillment) => fulfillment.buy_amount(prices),
            Trade::Jit(jit) => jit.buy_amount(prices),
        }
    }

    pub fn custom_prices(
        &self,
        prices: &ClearingPrices,
    ) -> Result<CustomClearingPrices, error::Math> {
        match self {
            Trade::Fulfillment(fulfillment) => fulfillment.custom_prices(prices),
            Trade::Jit(jit) => jit.custom_prices(prices),
        }
    }

    pub fn receiver(&self) -> eth::Address {
        match self {
            Trade::Fulfillment(fulfillment) => fulfillment.order().receiver(),
            Trade::Jit(jit) => jit.order().receiver,
        }
    }
}

/// A trade which fulfills an order from the auction.
#[derive(Debug, Clone)]
pub struct Fulfillment {
    order: competition::Order,
    /// The amount executed by this fulfillment. See [`order::Partial`]. If the
    /// order is not partial, the executed amount must equal the amount from the
    /// order.
    executed: order::TargetAmount,
    /// The fee that is charged to the user for executing the order, in sell
    /// token.
    fee: order::SellAmount,
}

impl Fulfillment {
    pub fn new(
        order: competition::Order,
        executed: order::TargetAmount,
        fee: order::SellAmount,
    ) -> Result<Self, error::Trade> {
        // If the order is partial, the total executed amount can be smaller
        // than the target amount. Otherwise, the executed amount must
        // be equal to the target amount.
        let valid_execution = {
            let fee = match order.side {
                order::Side::Buy => order::TargetAmount::default(),
                order::Side::Sell => order::TargetAmount(fee.0),
            };

            let executed_with_fee = order::TargetAmount(
                executed
                    .0
                    .checked_add(fee.0)
                    .ok_or(error::Trade::InvalidExecutedAmount)?,
            );
            match order.partial {
                order::Partial::Yes { available } => executed_with_fee <= available,
                order::Partial::No => executed_with_fee == order.target(),
            }
        };

        if valid_execution {
            Ok(Self {
                order,
                executed,
                fee,
            })
        } else {
            Err(error::Trade::InvalidExecutedAmount)
        }
    }

    pub fn order(&self) -> &competition::Order {
        &self.order
    }

    pub fn executed(&self) -> order::TargetAmount {
        self.executed
    }

    /// Returns the effectively paid fee from the user's perspective
    /// considering their signed order and the uniform clearing prices
    pub fn fee(&self) -> order::SellAmount {
        self.fee
    }

    /// The effective amount that left the user's wallet including all fees.
    pub fn sell_amount(&self, prices: &ClearingPrices) -> Result<eth::TokenAmount, error::Math> {
        let before_fee = match self.order.side {
            order::Side::Sell => self.executed.0,
            order::Side::Buy => self
                .executed
                .0
                .checked_mul(prices.buy)
                .ok_or(Math::Overflow)?
                .checked_div(prices.sell)
                .ok_or(Math::DivisionByZero)?,
        };

        Ok(eth::TokenAmount(
            before_fee.checked_add(self.fee().0).ok_or(Math::Overflow)?,
        ))
    }

    /// The effective amount the user received after all fees.
    ///
    /// Settlement contract uses `ceil` division for buy amount calculation.
    pub fn buy_amount(&self, prices: &ClearingPrices) -> Result<eth::TokenAmount, error::Math> {
        let amount = match self.order.side {
            order::Side::Buy => self.executed.0,
            order::Side::Sell => self
                .executed
                .0
                .checked_mul(prices.sell)
                .ok_or(Math::Overflow)?
                .checked_ceil_div(&prices.buy)
                .ok_or(Math::DivisionByZero)?,
        };
        Ok(eth::TokenAmount(amount))
    }

    /// Computes custom clearing prices for this trade.
    ///
    /// Note: This function relies on `sell_amount()` and `buy_amount()` to
    /// correctly incorporate all adjustments (fees). No additional
    /// modifications are applied here.
    pub fn custom_prices(
        &self,
        prices: &ClearingPrices,
    ) -> Result<CustomClearingPrices, error::Math> {
        Ok(CustomClearingPrices {
            sell: self.buy_amount(prices)?.into(),
            buy: self.sell_amount(prices)?.into(),
        })
    }

    /// Returns the surplus denominated in the surplus token.
    ///
    /// The surplus token is the buy token for a sell order and sell token for a
    /// buy order.
    pub fn surplus_over_reference_price(
        &self,
        limit_sell: eth::U256,
        limit_buy: eth::U256,
        prices: ClearingPrices,
    ) -> Result<eth::TokenAmount, error::Trade> {
        let executed = self.executed().0;
        let executed_sell_amount = match self.order().side {
            Side::Buy => {
                // How much `sell_token` we need to sell to buy `executed`
                // amount of `buy_token`
                executed
                    .checked_mul(prices.buy)
                    .ok_or(Math::Overflow)?
                    .checked_div(prices.sell)
                    .ok_or(Math::DivisionByZero)?
            }
            Side::Sell => executed,
        };
        // Sell slightly more `sell_token` to capture the fee
        let executed_sell_amount_with_fee = executed_sell_amount
            // the fee is always expressed in sell token
            .checked_add(self.fee().0)
            .ok_or(Math::Overflow)?;
        let surplus = match self.order().side {
            Side::Buy => {
                // Scale to support partially fillable orders
                let limit_sell_amount = limit_sell
                    .checked_mul(executed)
                    .ok_or(Math::Overflow)?
                    .checked_div(limit_buy)
                    .ok_or(Math::DivisionByZero)?;
                // Remaining surplus after fees
                // Do not return error if `checked_sub` fails because violated
                // limit prices will be caught by simulation
                limit_sell_amount
                    .checked_sub(executed_sell_amount_with_fee)
                    .unwrap_or(eth::U256::ZERO)
            }
            Side::Sell => {
                // Scale to support partially fillable orders

                // `checked_ceil_div`` to be consistent with how settlement
                // contract calculates traded buy amounts
                // smallest allowed executed_buy_amount per settlement contract
                // is executed_sell_amount *
                // ceil(price_limits.buy / price_limits.sell)
                let limit_buy_amount = limit_buy
                    .checked_mul(executed_sell_amount_with_fee)
                    .ok_or(Math::Overflow)?
                    .checked_ceil_div(&limit_sell)
                    .ok_or(Math::DivisionByZero)?;
                // How much `buy_token` we get for `executed` amount of
                // `sell_token`
                let executed_buy_amount = executed
                    .checked_mul(prices.sell)
                    .ok_or(Math::Overflow)?
                    .checked_ceil_div(&prices.buy)
                    .ok_or(Math::DivisionByZero)?;
                // Remaining surplus after fees
                // Do not return error if `checked_sub` fails because violated
                // limit prices will be caught by simulation
                executed_buy_amount
                    .checked_sub(limit_buy_amount)
                    .unwrap_or(eth::U256::ZERO)
            }
        };
        Ok(surplus.into())
    }
}

/// Uniform clearing prices at which the trade was executed.
#[derive(Debug, Clone, Copy)]
pub struct ClearingPrices {
    pub sell: eth::U256,
    pub buy: eth::U256,
}

/// Custom clearing prices at which the trade was executed.
///
/// These prices differ from uniform clearing prices, in that they are adjusted
/// to account for all fees (gas cost and protocol fees).
///
/// These prices determine the actual traded amounts from the user perspective.
#[derive(Debug, Clone)]
pub struct CustomClearingPrices {
    pub sell: eth::U256,
    pub buy: eth::U256,
}

impl CustomClearingPrices {
    /// Checks the signed limit with the same checked products as
    /// GPv2Settlement. Comparing rounded execution amounts can hide a
    /// limit-price violation.
    pub fn satisfies_limit(&self, limits: competition::PriceLimits) -> bool {
        let Some(sell_value) = limits.sell.0.checked_mul(self.sell) else {
            return false;
        };
        let Some(buy_value) = limits.buy.0.checked_mul(self.buy) else {
            return false;
        };
        sell_value >= buy_value
    }
}

/// A trade which adds a JIT order. See [`order::Jit`].
#[derive(Debug, Clone)]
pub struct Jit {
    order: order::Jit,
    /// The amount executed by this JIT trade. See
    /// [`order::Jit::partially_fillable`]. If the order is not
    /// partially fillable, the executed amount must equal the amount from the
    /// order.
    executed: order::TargetAmount,
    fee: order::SellAmount,
}

impl Jit {
    pub fn new(
        order: order::Jit,
        executed: order::TargetAmount,
        fee: order::SellAmount,
    ) -> Result<Self, error::Trade> {
        // If the order is partial, the total executed amount can be smaller
        // than the target amount. Otherwise, the executed amount must
        // be equal to the target amount.
        let fee_target_amount = match order.side {
            order::Side::Buy => order::TargetAmount::default(),
            order::Side::Sell => fee.0.into(),
        };

        let executed_with_fee = order::TargetAmount(
            executed
                .0
                .checked_add(fee_target_amount.into())
                .ok_or(error::Trade::InvalidExecutedAmount)?,
        );

        // If the order is partially fillable, the executed amount can be
        // smaller than the target amount. Otherwise, the executed
        // amount must be equal to the target amount.
        let is_valid = match order.partially_fillable() {
            order::Partial::Yes { available } => executed_with_fee <= available,
            order::Partial::No => executed_with_fee == order.target(),
        };

        if is_valid {
            Ok(Self {
                order,
                executed,
                fee,
            })
        } else {
            Err(error::Trade::InvalidExecutedAmount)
        }
    }

    pub fn order(&self) -> &order::Jit {
        &self.order
    }

    pub fn executed(&self) -> order::TargetAmount {
        self.executed
    }

    pub fn executed_buy(&self) -> Result<eth::TokenAmount, Math> {
        Ok(match self.order().side {
            Side::Buy => self.executed().into(),
            Side::Sell => (self
                .executed()
                .0
                .checked_add(self.fee().0)
                .ok_or(Math::Overflow)?)
            .checked_mul(self.order.buy.amount.0)
            .ok_or(Math::Overflow)?
            .checked_ceil_div(&self.order.sell.amount.0)
            .ok_or(Math::DivisionByZero)?
            .into(),
        })
    }

    pub fn executed_sell(&self) -> Result<eth::TokenAmount, Math> {
        Ok(match self.order().side {
            Side::Buy => self
                .executed()
                .0
                .checked_mul(self.order.sell.amount.0)
                .ok_or(Math::Overflow)?
                .checked_div(self.order.buy.amount.0)
                .ok_or(Math::DivisionByZero)?
                .into(),
            Side::Sell => self
                .executed()
                .0
                .checked_add(self.fee().0)
                .ok_or(Math::Overflow)?
                .into(),
        })
    }

    pub fn fee(&self) -> order::SellAmount {
        self.fee
    }

    /// The effective amount that left the user's wallet including all fees.
    pub fn sell_amount(&self, prices: &ClearingPrices) -> Result<eth::TokenAmount, Math> {
        let before_fee = match self.order.side {
            Side::Sell => self.executed.0,
            Side::Buy => self
                .executed
                .0
                .checked_mul(prices.buy)
                .ok_or(Math::Overflow)?
                .checked_div(prices.sell)
                .ok_or(Math::DivisionByZero)?,
        };
        Ok(eth::TokenAmount(
            before_fee.checked_add(self.fee.0).ok_or(Math::Overflow)?,
        ))
    }

    /// The effective amount the user received after all fees.
    pub fn buy_amount(&self, prices: &ClearingPrices) -> Result<eth::TokenAmount, Math> {
        let amount = match self.order.side {
            Side::Buy => self.executed.0,
            Side::Sell => self
                .executed
                .0
                .checked_mul(prices.sell)
                .ok_or(Math::Overflow)?
                .checked_ceil_div(&prices.buy)
                .ok_or(Math::DivisionByZero)?,
        };
        Ok(eth::TokenAmount(amount))
    }

    pub fn custom_prices(
        &self,
        prices: &ClearingPrices,
    ) -> Result<CustomClearingPrices, error::Math> {
        Ok(CustomClearingPrices {
            sell: self.buy_amount(prices)?.into(),
            buy: self.sell_amount(prices)?.into(),
        })
    }
}

/// The amounts executed by a trade.
#[derive(Debug, Clone, Copy)]
pub struct Execution {
    /// The total amount being sold.
    pub sell: eth::Asset,
    /// The total amount being bought.
    pub buy: eth::Asset,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn captured_fee_adjusted_prices_match_settlement_limit_checks() {
        use {number::serialization::HexOrDecimalU256, serde_with::serde_as};

        #[derive(serde::Deserialize)]
        struct Fixtures {
            invalid_count: usize,
            successful_trade_count: usize,
            cases: Vec<Case>,
        }
        #[serde_as]
        #[derive(serde::Deserialize)]
        struct Case {
            #[serde_as(as = "HexOrDecimalU256")]
            sell_amount: eth::U256,
            #[serde_as(as = "HexOrDecimalU256")]
            buy_amount: eth::U256,
            #[serde_as(as = "HexOrDecimalU256")]
            sell_price: eth::U256,
            #[serde_as(as = "HexOrDecimalU256")]
            buy_price: eth::U256,
            valid: bool,
        }
        let fixtures: Fixtures =
            serde_json::from_str(include_str!("fixtures/fee_adjusted_clearing_prices.json"))
                .unwrap();
        assert_eq!(fixtures.invalid_count, 32);
        assert_eq!(fixtures.successful_trade_count, 7);
        assert_eq!(
            fixtures.cases.iter().filter(|case| !case.valid).count(),
            fixtures.invalid_count
        );
        assert_eq!(
            fixtures.cases.len(),
            fixtures.invalid_count + fixtures.successful_trade_count
        );
        for (index, case) in fixtures.cases.into_iter().enumerate() {
            let prices = CustomClearingPrices {
                sell: case.sell_price,
                buy: case.buy_price,
            };
            assert_eq!(
                prices.satisfies_limit(competition::PriceLimits {
                    sell: case.sell_amount.into(),
                    buy: case.buy_amount.into(),
                }),
                case.valid,
                "fixture {index}"
            );
        }
    }

    #[test]
    fn signed_limit_checks_use_exact_products() {
        for (sell_price, buy_price, valid) in [
            (200, 100, true), // exactly the signed limit
            (201, 100, true),
            (199, 100, false), // one buy-token atom short
            (200, 101, false), // one sell-token atom too much
            (100, 50, true),   // partial fill at the same ratio
            (99, 50, false),
            (0, 100, false),
            (200, 0, true), // a buy order can receive tokens for free
            (0, 0, true),   // satisfies the limit; division validity is a separate check
        ] {
            let prices = CustomClearingPrices {
                sell: eth::U256::from(sell_price),
                buy: eth::U256::from(buy_price),
            };
            assert_eq!(
                prices.satisfies_limit(competition::PriceLimits {
                    sell: eth::U256::from(100).into(),
                    buy: eth::U256::from(200).into(),
                }),
                valid,
                "{prices:?}"
            );
        }
    }

    #[test]
    fn rounded_partial_fill_does_not_hide_invalid_price_ratio() {
        // A one-atom fill rounds up to one bought atom, meeting the rounded
        // pro-rata minimum. GPv2 still rejects the prices: 3 * 1 < 2 * 2.
        assert!(
            !CustomClearingPrices {
                sell: eth::U256::from(1),
                buy: eth::U256::from(2),
            }
            .satisfies_limit(competition::PriceLimits {
                sell: eth::U256::from(3).into(),
                buy: eth::U256::from(2).into(),
            })
        );
    }

    #[test]
    fn overflowing_limit_products_are_not_encodable() {
        for (sell_limit, buy_limit, sell_price, buy_price) in [
            (
                eth::U256::MAX,
                eth::U256::ONE,
                eth::U256::from(2),
                eth::U256::ONE,
            ),
            (
                eth::U256::ONE,
                eth::U256::MAX,
                eth::U256::ONE,
                eth::U256::from(2),
            ),
            (
                eth::U256::MAX,
                eth::U256::MAX,
                eth::U256::from(2),
                eth::U256::from(2),
            ),
        ] {
            assert!(
                !CustomClearingPrices {
                    sell: sell_price,
                    buy: buy_price,
                }
                .satisfies_limit(competition::PriceLimits {
                    sell: sell_limit.into(),
                    buy: buy_limit.into(),
                })
            );
        }
        // Large values alone are not a reason to reject a valid price.
        assert!(
            CustomClearingPrices {
                sell: eth::U256::ONE,
                buy: eth::U256::ONE,
            }
            .satisfies_limit(competition::PriceLimits {
                sell: eth::U256::MAX.into(),
                buy: eth::U256::MAX.into(),
            })
        );
    }
}
