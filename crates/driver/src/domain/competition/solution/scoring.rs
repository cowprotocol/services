//! Scoring of a solution.
//!
//! Scoring is done on a solution that is identical to the one that will appear
//! onchain. This means that all fees are already applied to the trades and the
//! executed amounts are adjusted to account for all fees (gas cost and protocol
//! fees). No further changes are expected to be done on solution by the driver
//! after scoring.

use {
    super::{
        error::Math,
        order::{self, Side},
        trade::CustomClearingPrices,
    },
    crate::domain::{
        competition::{
            PriceLimits,
            auction,
            order::FeePolicy,
            solution::{
                error,
                fee::{self, adjust_quote_to_order_limits},
            },
        },
        eth,
    },
    alloy::primitives::ruint::UintTryFrom,
    number::u256_ext::U256Ext,
};

pub fn compute_score(
    trades: &[Trade],
    native_prices: &auction::Prices,
) -> Result<eth::Ether, Error> {
    trades.iter().map(|trade| trade.score(native_prices)).sum()
}

// Trade represents a single trade in a settlement.
//
// It contains values as expected by the settlement contract. That means that
// clearing prices are adjusted to account for all fees (gas cost and protocol
// fees). Also, executed amount contains the fees for sell order.
#[derive(Debug, Clone)]
pub struct Trade {
    /// signed sell token parameters of the order (i.e. limit price)
    signed_sell: eth::Asset,
    /// signed buy token parameters of the order (i.e. limit price)
    signed_buy: eth::Asset,
    side: Side,
    executed: order::TargetAmount,
    /// Price at which the order gets filled. This is based on the solution's
    /// price vector and the necessary adjustements to incorporate fees.
    custom_price: CustomClearingPrices,
    policies: Vec<FeePolicy>,
}

impl Trade {
    pub fn new(
        signed_sell: eth::Asset,
        signed_buy: eth::Asset,
        side: Side,
        executed: order::TargetAmount,
        custom_price: CustomClearingPrices,
        policies: Vec<FeePolicy>,
    ) -> Self {
        Self {
            signed_sell,
            signed_buy,
            side,
            executed,
            custom_price,
            policies,
        }
    }

    /// Score defined as (surplus + protocol fees) first converted to buy
    /// amounts and then converted to the native token.
    ///
    /// [CIP-38](https://forum.cow.fi/t/cip-38-solver-computed-fees-rank-by-surplus/2061>) as the
    /// base of the score computation.
    /// [Draft CIP](https://forum.cow.fi/t/cip-draft-updating-score-definition-for-buy-orders/2930)
    /// as the latest revision to avoid edge cases for certain buy orders./
    ///
    /// Denominated in NATIVE token
    fn score(&self, native_prices: &auction::Prices) -> Result<eth::Ether, Error> {
        tracing::debug!("Scoring trade {:?}", self);
        let native_price_buy = native_prices
            .get(&self.signed_buy.token)
            .ok_or(Error::MissingPrice(self.signed_buy.token))?;

        let surplus_in_surplus_token = self
            .user_surplus()?
            .0
            .checked_add(self.fees()?.0)
            .ok_or(Error::Math(Math::Overflow))?;

        let score = match self.side {
            // `surplus` of sell orders is already in buy tokens so we simply convert it to ETH
            Side::Sell => native_price_buy.in_eth(eth::TokenAmount(surplus_in_surplus_token)),
            Side::Buy => {
                // `surplus` of buy orders is in sell tokens. We start with following formula:
                // buy_amount / sell_amount == buy_price / sell_price
                //
                // since `surplus` of buy orders is in sell tokens we convert to buy amount via:
                // buy_amount == (buy_price / sell_price) * surplus
                //
                // to avoid loss of precision because we work with integers we first multiply
                // and then divide:
                // buy_amount = surplus * buy_price / sell_price
                let surplus_in_buy_tokens = surplus_in_surplus_token
                    .widening_mul(self.signed_buy.amount.0)
                    .checked_div(alloy::primitives::U512::from(self.signed_sell.amount.0))
                    .ok_or(Error::Math(Math::DivisionByZero))?;
                let surplus_in_buy_tokens = eth::U256::uint_try_from(surplus_in_buy_tokens)
                    .map_err(|_| Error::Math(Math::Overflow))?;

                // Afterwards we convert the buy token surplus to the native token.
                native_price_buy.in_eth(surplus_in_buy_tokens.into())
            }
        };
        Ok(score)
    }

    /// Surplus based on custom clearing prices returns the surplus after all
    /// fees have been applied and calculated over the price limits.
    fn surplus_over(&self, price_limits: PriceLimits) -> Result<eth::SurplusTokenAmount, Math> {
        match self.side {
            Side::Buy => {
                // scale limit sell to support partially fillable orders
                let limit_sell = price_limits
                    .sell
                    .0
                    .checked_mul(self.executed.into())
                    .ok_or(Math::Overflow)?
                    .checked_div(price_limits.buy.0)
                    .ok_or(Math::DivisionByZero)?;
                let sold = self
                    .executed
                    .0
                    .checked_mul(self.custom_price.buy)
                    .ok_or(Math::Overflow)?
                    .checked_div(self.custom_price.sell)
                    .ok_or(Math::DivisionByZero)?;
                limit_sell.checked_sub(sold).ok_or(Math::Negative)
            }
            Side::Sell => {
                // scale limit buy to support partially fillable orders

                // `checked_ceil_div`` to be consistent with how settlement contract calculates
                // traded buy amounts
                // smallest allowed executed_buy_amount per settlement contract is
                // executed_sell_amount * ceil(price_limits.buy / price_limits.sell)
                let limit_buy = self
                    .executed
                    .0
                    .checked_mul(price_limits.buy.0)
                    .ok_or(Math::Overflow)?
                    .checked_ceil_div(&price_limits.sell.0)
                    .ok_or(Math::DivisionByZero)?;
                let bought = self
                    .executed
                    .0
                    .checked_mul(self.custom_price.sell)
                    .ok_or(Math::Overflow)?
                    .checked_ceil_div(&self.custom_price.buy)
                    .ok_or(Math::DivisionByZero)?;
                bought.checked_sub(limit_buy).ok_or(Math::Negative)
            }
        }
        .map(eth::SurplusTokenAmount)
    }

    /// Protocol fees are defined by fee policies attached to the order.
    fn fees(&self) -> Result<eth::SurplusTokenAmount, Error> {
        let mut current_trade = self.clone();
        let mut total = eth::SurplusTokenAmount::default();
        for protocol_fee in self.policies.iter().rev() {
            total = total
                .0
                .checked_add(current_trade.protocol_fee(protocol_fee)?.0)
                .ok_or(Error::Math(Math::Overflow))?
                .into();
            current_trade.custom_price = self.calculate_custom_prices(total)?;
        }
        Ok(total)
    }

    /// The effective amount that left the user's wallet including all fees.
    ///
    /// Note how the `executed` amount is used to build actual traded amounts.
    fn sell_amount(&self) -> Result<eth::TokenAmount, error::Math> {
        Ok(match self.side {
            order::Side::Sell => self.executed.0,
            order::Side::Buy => self
                .executed
                .0
                .checked_mul(self.custom_price.buy)
                .ok_or(Math::Overflow)?
                .checked_div(self.custom_price.sell)
                .ok_or(Math::DivisionByZero)?,
        }
        .into())
    }

    /// The effective amount the user received after all fees.
    ///
    /// Note how the `executed` amount is used to build actual traded amounts.
    ///
    /// Settlement contract uses `ceil` division for buy amount calculation.
    fn buy_amount(&self) -> Result<eth::TokenAmount, error::Math> {
        Ok(match self.side {
            order::Side::Sell => self
                .executed
                .0
                .checked_mul(self.custom_price.sell)
                .ok_or(Math::Overflow)?
                .checked_ceil_div(&self.custom_price.buy)
                .ok_or(Math::DivisionByZero)?,
            order::Side::Buy => self.executed.0,
        }
        .into())
    }

    /// Derive new custom prices (given the current custom prices) to exclude
    /// the protocol fee from the trade.
    ///
    /// Note how the custom prices are expressed over actual traded amounts.
    pub fn calculate_custom_prices(
        &self,
        protocol_fee: eth::SurplusTokenAmount,
    ) -> Result<CustomClearingPrices, error::Math> {
        Ok(CustomClearingPrices {
            sell: match self.side {
                Side::Sell => self
                    .buy_amount()?
                    .0
                    .checked_add(protocol_fee.0)
                    .ok_or(Math::Overflow)?,
                Side::Buy => self.buy_amount()?.0,
            },
            buy: match self.side {
                Side::Sell => self.sell_amount()?.0,
                Side::Buy => self
                    .sell_amount()?
                    .0
                    .checked_sub(protocol_fee.0)
                    .ok_or(Math::Negative)?,
            },
        })
    }

    /// Protocol fee is defined by a fee policy attached to the order.
    fn protocol_fee(&self, fee_policy: &FeePolicy) -> Result<eth::SurplusTokenAmount, Error> {
        let amount = match fee_policy {
            FeePolicy::Surplus {
                factor,
                max_volume_factor,
            } => {
                let surplus = self.user_surplus()?;
                std::cmp::min(
                    self.fee(surplus, *factor)?,
                    self.volume_fee(*max_volume_factor)?,
                )
            }
            FeePolicy::PriceImprovement {
                factor,
                max_volume_factor,
                quote,
            } => {
                let price_improvement = self.price_improvement(quote)?;
                std::cmp::min(
                    self.fee(price_improvement, *factor)?,
                    self.volume_fee(*max_volume_factor)?,
                )
            }
            FeePolicy::Volume { factor } => self.volume_fee(*factor)?,
        };
        Ok(eth::SurplusTokenAmount(amount.0))
    }

    fn price_improvement(&self, quote: &order::Quote) -> Result<eth::SurplusTokenAmount, Error> {
        let quote = adjust_quote_to_order_limits(
            fee::Order {
                sell_amount: self.signed_sell.amount.0,
                buy_amount: self.signed_buy.amount.0,
                side: self.side,
            },
            fee::Quote {
                sell_amount: quote.sell.amount.0,
                buy_amount: quote.buy.amount.0,
                fee_amount: quote.fee.amount.0,
            },
        )?;
        let surplus = self.surplus_over(quote);
        // negative surplus is not error in this case, as solutions often have no
        // improvement over quote which results in negative surplus
        if let Err(Math::Negative) = surplus {
            return Ok(eth::SurplusTokenAmount(eth::U256::ZERO));
        }
        Ok(surplus?)
    }

    /// Amount of value the user got above the order's limit price
    /// denominated in the surplus token.
    fn user_surplus(&self) -> Result<eth::SurplusTokenAmount, Error> {
        let limit_price = PriceLimits {
            sell: self.signed_sell.amount,
            buy: self.signed_buy.amount,
        };
        Ok(self.surplus_over(limit_price)?)
    }

    /// Protocol fee as a cut of surplus.
    fn fee(
        &self,
        surplus: eth::SurplusTokenAmount,
        factor: f64,
    ) -> Result<eth::SurplusTokenAmount, Error> {
        // Surplus fee is specified as a `factor` from raw surplus (before fee). Since
        // this module works with trades that already have the protocol fee applied, we
        // need to calculate the protocol fee as an observation of the eventually traded
        // amounts using a different factor `factor'`.
        //
        // The protocol fee before being applied is:
        //    fee = surplus_before_fee * factor
        // The protocol fee after being applied is:
        //    fee = surplus_after_fee * factor'
        // Also:
        //    surplus_after_fee = surplus_before_fee - fee
        // So:
        //    factor' = fee / surplus_after_fee = fee / (surplus_before_fee -
        // fee) = fee / ((fee / factor) - fee) = factor / (1 - factor)
        //
        // Finally:
        //     fee = surplus_after_fee * factor / (1 - factor)
        let multiplied = surplus
            .0
            .checked_mul_f64(factor / (1.0 - factor))
            .ok_or(Error::Math(Math::Overflow))?;
        Ok(multiplied.into())
    }

    /// Protocol fee as a cut of the trade volume.
    fn volume_fee(&self, factor: f64) -> Result<eth::SurplusTokenAmount, Error> {
        // Volume fee is specified as a `factor` from raw volume (before fee). Since
        // this module works with trades that already have the protocol fee applied, we
        // need to calculate the protocol fee as an observation of a the eventually
        // traded amount using a different factor `factor'` .
        //
        // The protocol fee before being applied is:
        // case Sell: fee = traded_buy_amount * factor, resulting in the REDUCED
        // buy amount
        // case Buy: fee = traded_sell_amount * factor, resulting in the INCREASED
        // sell amount
        //
        // The protocol fee after being applied is:
        // case Sell: fee = traded_buy_amount' * factor',
        // case Buy: fee = traded_sell_amount' * factor',
        //
        // Also:
        // case Sell: traded_buy_amount' = traded_buy_amount - fee
        // case Buy: traded_sell_amount' = traded_sell_amount + fee
        //
        // So:
        // case Sell: factor' = fee / (traded_buy_amount - fee) = fee / (fee /
        // factor - fee) = factor / (1 - factor)
        // case Buy: factor' = fee / (traded_sell_amount + fee) = fee / (fee /
        // factor + fee) = factor / (1 + factor)
        //
        // Finally:
        // case Sell: fee = traded_buy_amount' * factor / (1 - factor)
        // case Buy: fee = traded_sell_amount' * factor / (1 + factor)
        let executed_in_surplus_token = match self.side {
            order::Side::Buy => eth::SurplusTokenAmount(self.sell_amount()?.0),
            order::Side::Sell => eth::SurplusTokenAmount(self.buy_amount()?.0),
        };
        let factor = match self.side {
            Side::Sell => factor / (1.0 - factor),
            Side::Buy => factor / (1.0 + factor),
        };

        let multiplied = executed_in_surplus_token
            .0
            .checked_mul_f64(factor)
            .ok_or(Error::Math(Math::Overflow))?;
        Ok(multiplied.into())
    }
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("missing native price for token {0:?}")]
    MissingPrice(eth::TokenAddress),
    #[error(transparent)]
    Math(#[from] Math),
    #[error("scoring: failed to calculate custom price for the applied fee policy {0:?}")]
    Scoring(#[source] error::Scoring),
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        alloy::primitives::{Address, U256, address},
        auction::Price,
        std::collections::HashMap,
    };

    /// Tests that the new score computation limits the score of certain
    /// buy orders to a reasonable amount.
    /// Data is based on this
    /// [auction](https://api.cow.fi/base/api/v1/solver_competition/by_tx_hash/0xe3ef02493255f17c0abd2ff88c34682d35f0de4f4875a4653104e3453473d8d9).
    #[test]
    fn score_problematic_buy_order() {
        const WETH: Address = address!("4200000000000000000000000000000000000006");
        const BNKR: Address = address!("22af33fe49fd1fa80c7149773dde5890d3c76f3b");

        // Buy order which results in an unreasonably high score
        // using the original scoring mechanism.
        let trade = Trade {
            signed_sell: eth::Asset {
                token: WETH.into(),
                amount: 9865986634773384514560000000000000u128.into(),
            },
            signed_buy: eth::Asset {
                token: BNKR.into(),
                amount: 4025333872768468868566822740u128.into(),
            },
            side: Side::Buy,
            executed: order::TargetAmount(U256::from(8050667745u128)),
            custom_price: CustomClearingPrices {
                sell: U256::from(874045870u128),
                buy: U256::from(8050667745u128),
            },
            policies: vec![],
        };

        let native_prices: HashMap<_, _> = [
            (
                WETH.into(),
                Price(eth::Ether(U256::from(1000000000000000000u128))),
            ),
            (BNKR.into(), Price(eth::Ether(U256::from(113181296327u128)))),
        ]
        .into_iter()
        .collect();

        let score = trade.score(&native_prices).unwrap();
        assert_eq!(score.0, U256::from(911));
    }
}
