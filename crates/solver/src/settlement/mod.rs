mod settlement_encoder;

use {
    crate::liquidity::Settleable,
    anyhow::Result,
    model::order::{Order, OrderKind},
    primitive_types::{H160, U256},
    shared::{
        conversions::U256Ext as _,
        encoded_settlement::{encode_trade, EncodedSettlement, EncodedTrade},
        http_solver::model::InternalizationStrategy,
    },
    std::collections::HashMap,
};

pub use self::settlement_encoder::{PricedTrade, SettlementEncoder};

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Trade {
    pub order: Order,
    pub executed_amount: U256,
    pub fee: U256,
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
    // Returns the executed fee amount (prorated of executed amount)
    // cf. https://github.com/cowprotocol/contracts/blob/v1.1.2/src/contracts/GPv2Settlement.sol#L383-L385
    fn executed_fee(&self) -> Option<U256> {
        self.scale_amount(self.order.data.fee_amount)
    }

    /// Scales the passed `amount` based on the `executed_amount`.
    fn scale_amount(&self, amount: U256) -> Option<U256> {
        match self.order.data.kind {
            model::order::OrderKind::Buy => amount
                .checked_mul(self.executed_amount)?
                .checked_div(self.order.data.buy_amount),
            model::order::OrderKind::Sell => amount
                .checked_mul(self.executed_amount)?
                .checked_div(self.order.data.sell_amount),
        }
    }

    /// Computes and returns the executed trade amounts given sell and buy
    /// prices.
    pub(crate) fn executed_amounts(
        &self,
        sell_price: U256,
        buy_price: U256,
    ) -> Option<TradeExecution> {
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

        Some(TradeExecution {
            sell_token: order.sell_token,
            buy_token: order.buy_token,
            sell_amount,
            buy_amount,
            fee_amount: self.executed_fee()?,
        })
    }
}

impl Trade {
    /// Encodes the settlement's order_trade as a tuple, as expected by the
    /// smart contract.
    pub(crate) fn encode(&self, sell_token_index: usize, buy_token_index: usize) -> EncodedTrade {
        encode_trade(
            &self.order.data,
            &self.order.signature,
            self.order.metadata.owner,
            sell_token_index,
            buy_token_index,
            &self.executed_amount,
        )
    }
}

#[derive(Debug, Clone, Default)]
pub struct Settlement {
    pub encoder: SettlementEncoder,
}

#[derive(Debug, Eq, PartialEq)]
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

    pub fn with_liquidity<L>(&mut self, liquidity: &L, execution: L::Execution) -> Result<()>
    where
        L: Settleable,
    {
        liquidity
            .settlement_handling()
            .encode(execution, &mut self.encoder)
    }

    /// Returns the clearing prices map.
    pub fn clearing_prices(&self) -> &HashMap<H160, U256> {
        self.encoder.clearing_prices()
    }

    /// Returns the clearing price for the specified token.
    ///
    /// Returns `None` if the token is not part of the settlement.
    #[cfg(test)]
    pub(crate) fn clearing_price(&self, token: H160) -> Option<U256> {
        self.clearing_prices().get(&token).copied()
    }

    /// Returns all orders included in the settlement.
    pub fn traded_orders(&self) -> impl Iterator<Item = &Order> + '_ {
        self.encoder.all_trades().map(|trade| &trade.data.order)
    }

    /// Returns an iterator of all executed trades.
    #[cfg(test)]
    pub fn trade_executions(&self) -> impl Iterator<Item = TradeExecution> + '_ {
        self.encoder.all_trades().map(|trade| {
            trade
                .executed_amounts()
                .expect("invalid trade was added to encoder")
        })
    }

    pub fn encode(self, internalization_strategy: InternalizationStrategy) -> EncodedSettlement {
        self.encoder.finish(internalization_strategy)
    }
}

#[cfg(test)]
pub mod tests {
    use {
        super::*,
        crate::liquidity::SettlementHandling,
        maplit::hashmap,
        model::order::{OrderClass, OrderData, OrderKind, OrderMetadata},
    };

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

    /// Helper function for creating a settlement for the specified prices and
    /// trades for testing objective value computations.
    fn test_settlement(prices: HashMap<H160, U256>, trades: Vec<Trade>) -> Settlement {
        Settlement {
            encoder: SettlementEncoder::with_trades(prices, trades),
        }
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
                        class: OrderClass::Limit,
                        ..Default::default()
                    },
                    ..Default::default()
                },
                executed_amount: 99_000_u128.into(),
                fee: 1_000_u128.into(),
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
