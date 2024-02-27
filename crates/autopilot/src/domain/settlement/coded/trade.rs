use {
    super::TradeFlags,
    crate::domain::{self, auction::order, eth, fee, settlement::surplus},
};

#[derive(Debug)]
pub struct Trade {
    pub sell: eth::Asset,
    pub buy: eth::Asset,
    pub receiver: eth::Address,
    pub valid_to: u32,
    pub app_data: order::AppDataHash,
    pub flags: TradeFlags,
    pub executed: eth::Asset,
    pub signature: order::Signature,

    /// [ Additional derived fields ]
    ///
    /// The order uid of the order associated with this trade.
    pub order_uid: domain::OrderUid,
    /// Derived from the settlement "clearing_prices" vector
    pub prices: Price,
}

impl Trade {
    /// Surplus based on uniform clearing prices returns the surplus without any
    /// fees applied.
    ///
    /// [ Denominated in surplus token ]
    pub fn surplus_before_fee(&self) -> Option<eth::Asset> {
        surplus::trade_surplus(
            self.flags.order_kind(),
            self.executed,
            self.sell,
            self.buy,
            &self.prices.uniform,
        )
    }

    /// Surplus based on custom clearing prices returns the surplus after all
    /// fees have been applied.
    ///
    /// [ Denominated in surplus token ]
    pub fn surplus(&self) -> Option<eth::Asset> {
        surplus::trade_surplus(
            self.flags.order_kind(),
            self.executed,
            self.sell,
            self.buy,
            &self.prices.custom,
        )
    }

    /// Fee is the difference between the surplus over uniform clearing prices
    /// and surplus over custom clearing prices.
    ///
    /// [ Denominated in surplus token ]
    fn fee(&self) -> Option<eth::Asset> {
        self.surplus_before_fee()
            .zip(self.surplus())
            .map(|(before, after)| eth::Asset {
                token: before.token,
                amount: before.amount.saturating_sub(*after.amount).into(),
            })
    }

    /// Fee is the difference between the surplus over uniform clearing prices
    /// and surplus over custom clearing prices.
    ///
    /// [ Denominated in sell token ]
    pub fn fee_in_sell_token(&self) -> Option<eth::Asset> {
        match self.flags.order_kind() {
            order::Kind::Buy => self.fee(),
            order::Kind::Sell => self.fee().map(|fee| eth::Asset {
                token: self.sell.token,
                amount: (*fee.amount * self.prices.uniform.buy / self.prices.uniform.sell).into(),
            }),
        }
    }

    /// Protocol fee is defined by fee policies attached to the order.
    ///
    /// [ Denominated in surplus token ]
    fn protocol_fee(&self, policies: &[fee::Policy]) -> Option<eth::Asset> {
        // TODO: support multiple fee policies
        if policies.len() > 1 {
            return None;
        }

        match policies.first()? {
            fee::Policy::Surplus {
                factor,
                max_volume_factor,
            } => Some(eth::Asset {
                token: match self.flags.order_kind() {
                    order::Kind::Sell => self.buy.token,
                    order::Kind::Buy => self.sell.token,
                },
                amount: std::cmp::min(
                    {
                        let original_surplus = apply_factor(*self.fee()?.amount, 1.0 - factor)?;
                        apply_factor(original_surplus, *factor)?
                    },
                    apply_factor(*self.executed.amount, *max_volume_factor)?, /* todo convert
                                                                               * executed amount
                                                                               * to surplus
                                                                               * token */
                )
                .into(),
            }),
            fee::Policy::PriceImprovement {
                factor: _,
                max_volume_factor: _,
                quote: _,
            } => todo!(),
            fee::Policy::Volume { factor: _ } => todo!(),
        }
    }

    /// CIP38 score defined as surplus + protocol fee
    ///
    /// [ Denominated in surplus token ]
    pub fn score(&self, policies: &[fee::Policy]) -> Option<eth::Asset> {
        // TODO: support multiple fee policies
        if policies.len() > 1 {
            return None;
        }

        self.surplus()
            .zip(self.protocol_fee(policies))
            .map(|(surplus, fee)| eth::Asset {
                token: surplus.token,
                amount: (*surplus.amount + *fee.amount).into(),
            })
    }
}

fn apply_factor(amount: eth::U256, factor: f64) -> Option<eth::U256> {
    Some(amount.checked_mul(eth::U256::from_f64_lossy(factor * 10000000000.))? / 10000000000u128)
}

#[derive(Debug)]
pub struct Price {
    pub uniform: ClearingPrices,
    /// Adjusted uniform prices to account for fees (gas cost and protocol fees)
    pub custom: ClearingPrices,
}

/// Uniform clearing prices at which the trade was executed.
#[derive(Debug, Clone, Copy)]
pub struct ClearingPrices {
    pub sell: eth::U256,
    pub buy: eth::U256,
}
