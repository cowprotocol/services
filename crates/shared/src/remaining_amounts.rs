use {
    anyhow::{Context, Result},
    model::order::{Order as ModelOrder, OrderKind},
    num::rational::Ratio,
    primitive_types::U256,
};

/// Calculates the remaining amounts for an order.
///
/// For example, when a sell order has half of its sell amount already executed
/// then the remaining buy and fee amounts are also half of their original.
///
/// Works like the smart contract by taking intermediate overflows into account
/// for partially fillable orders.
pub struct Remaining {
    /// The ratio of a order that is available based on order execution. That
    /// is, if a partially fillable order selling 100 DAI already executed
    /// 75, this ratio will be `25/100`.
    ///
    /// This is always `0/1` or `1/1` for fill or kill orders.
    execution: Ratio<U256>,

    /// The ratio of a order that is available based on user balance. That is,
    /// if a partially fillable order selling 90 DAI with a fee of 10 DAI where
    /// the owner has a balance of 25, then this ratio will be `25/100`.
    ///
    /// This is always `0/1` or `1/1` for fill or kill orders.
    ///
    /// Note that this ratio is kept separate from `execution`. This is done
    /// in order to respect Smart Contract overflow semantics. In particular,
    /// scaling amounts for partially executed orders can result in overflows
    /// in the Settlement contract, however, the Settlement Smart Contract does
    /// not concern itself with overflows WRT to user balances, so we compute
    /// remaining order amount from user balances separately and without
    /// overflow checks in the intermediate operations.
    balance: Ratio<U256>,
}

pub struct Order {
    pub kind: OrderKind,
    pub buy_amount: U256,
    pub sell_amount: U256,
    pub fee_amount: U256,
    // For a buy order this is in the buy token and for a sell order in the sell token and
    // excluding the fee amount.
    pub executed_amount: U256,
    pub partially_fillable: bool,
}

impl From<&ModelOrder> for Order {
    fn from(o: &ModelOrder) -> Self {
        Self {
            kind: o.data.kind,
            buy_amount: *o.data.buy_amount,
            sell_amount: *o.data.sell_amount,
            fee_amount: *o.data.fee_amount,
            executed_amount: match o.data.kind {
                // A real buy order cannot execute more than U256::MAX so in order to make this
                // function infallible we treat a larger amount as a full execution.
                OrderKind::Buy => {
                    number::conversions::big_uint_to_u256(&o.metadata.executed_buy_amount)
                        .unwrap_or(*o.data.buy_amount)
                }
                OrderKind::Sell => *o.metadata.executed_sell_amount_before_fees,
            },
            partially_fillable: o.data.partially_fillable,
        }
    }
}

impl From<ModelOrder> for Order {
    fn from(o: ModelOrder) -> Self {
        (&o).into()
    }
}

impl Remaining {
    /// Returns a ratio of an order assuming it has sufficient balance.
    pub fn from_order(order: &Order) -> Result<Self> {
        Self::from_order_with_balance(order, U256::MAX)
    }

    /// Returns a ratio of an order with the specified available balance.
    pub fn from_order_with_balance(order: &Order, sell_balance: U256) -> Result<Self> {
        let total = match order.kind {
            OrderKind::Buy => order.buy_amount,
            OrderKind::Sell => order.sell_amount,
        };

        if order.partially_fillable {
            let execution = Ratio::new_raw(
                total
                    .checked_sub(order.executed_amount)
                    .context("executed larger than total")?,
                total,
            );

            let sell_amount = ratio::scalar_mul(&execution, order.sell_amount)
                .context("overflow scaling sell amount for execution")?;
            let fee_amount = ratio::scalar_mul(&execution, order.fee_amount)
                .context("overflow scaling fee amount for execution")?;

            let need = sell_amount
                .checked_add(fee_amount)
                .context("partially fillable need calculation overflow")?;

            let balance = if sell_balance < need {
                Ratio::new_raw(sell_balance, need)
            } else {
                ratio::one()
            };

            Ok(Self { execution, balance })
        } else {
            let sell = order
                .sell_amount
                .checked_add(order.fee_amount)
                .context("overflow sell + fee amount")?;

            let execution = if order.executed_amount.is_zero() {
                ratio::one()
            } else {
                ratio::zero()
            };

            let balance = if sell_balance >= sell {
                ratio::one()
            } else {
                ratio::zero()
            };

            Ok(Self { execution, balance })
        }
    }

    /// Returns Err if the contract would error due to intermediate overflow.
    pub fn remaining(&self, total: U256) -> Result<U256> {
        ratio::full_scalar_mul(
            &self.balance,
            ratio::scalar_mul(&self.execution, total)
                .context("overflow scaling for order execution")?,
        )
        .context("overflow scaling for available balance")
    }
}

mod ratio {
    use {ethcontract::U256, num::rational::Ratio};

    pub fn one() -> Ratio<U256> {
        Ratio::new_raw(1.into(), 1.into())
    }

    pub fn zero() -> Ratio<U256> {
        Ratio::new_raw(0.into(), 1.into())
    }

    /// Multiplies a ratio by a scalar, returning `None` if the result or any
    /// intermediate operation would overflow a `U256`.
    pub fn scalar_mul(ratio: &Ratio<U256>, scalar: U256) -> Option<U256> {
        scalar
            .checked_mul(*ratio.numer())?
            .checked_div(*ratio.denom())
    }

    /// Multiplies a ratio by a scalar, returning `None` only if the result
    /// would overflow a `U256`, but intermediate operations are allowed to
    /// overflow.
    pub fn full_scalar_mul(ratio: &Ratio<U256>, scalar: U256) -> Option<U256> {
        scalar
            .full_mul(*ratio.numer())
            .checked_div(ratio.denom().into())?
            .try_into()
            .ok()
    }
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        model::order::{OrderData, OrderMetadata},
        num::BigUint,
    };

    #[test]
    fn computes_remaining_order_amounts() {
        // For fill-or-kill orders, we don't overflow even for very large buy
        // orders (where `{sell,fee}_amount * buy_amount` would overflow).
        let order = ModelOrder {
            data: OrderData {
                sell_amount: 1000_u32.into(),
                buy_amount: U256::MAX.into(),
                fee_amount: 337_u32.into(),
                kind: OrderKind::Buy,
                partially_fillable: false,
                ..Default::default()
            },
            metadata: OrderMetadata {
                full_fee_amount: 42_u32.into(),
                ..Default::default()
            },
            ..Default::default()
        }
        .into();
        let remaining = Remaining::from_order(&order).unwrap();
        assert_eq!(remaining.remaining(1000.into()).unwrap(), 1000.into());
        assert_eq!(remaining.remaining(U256::MAX).unwrap(), U256::MAX);

        // For partially fillable orders that are untouched, returns the full
        // order amounts.
        let order = ModelOrder {
            data: OrderData {
                sell_amount: 10_u32.into(),
                buy_amount: 11_u32.into(),
                fee_amount: 12_u32.into(),
                kind: OrderKind::Sell,
                partially_fillable: true,
                ..Default::default()
            },
            metadata: OrderMetadata {
                executed_sell_amount_before_fees: 0_u32.into(),
                full_fee_amount: 13_u32.into(),
                ..Default::default()
            },
            ..Default::default()
        }
        .into();
        let remaining = Remaining::from_order(&order).unwrap();
        assert_eq!(remaining.remaining(10.into()).unwrap(), 10.into());
        assert_eq!(remaining.remaining(13.into()).unwrap(), 13.into());

        // Scales amounts by how much has been executed. Rounds down like the
        // settlement contract.
        let order = ModelOrder {
            data: OrderData {
                sell_amount: 100_u32.into(),
                buy_amount: 100_u32.into(),
                fee_amount: 101_u32.into(),
                kind: OrderKind::Sell,
                partially_fillable: true,
                ..Default::default()
            },
            metadata: OrderMetadata {
                executed_sell_amount_before_fees: 90_u32.into(),
                full_fee_amount: 200_u32.into(),
                ..Default::default()
            },
            ..Default::default()
        }
        .into();
        let remaining = Remaining::from_order(&order).unwrap();
        assert_eq!(remaining.remaining(100.into()).unwrap(), 10.into());
        assert_eq!(remaining.remaining(101.into()).unwrap(), 10.into());
        assert_eq!(remaining.remaining(200.into()).unwrap(), 20.into());

        let order = ModelOrder {
            data: OrderData {
                sell_amount: 100_u32.into(),
                buy_amount: 10_u32.into(),
                fee_amount: 101_u32.into(),
                kind: OrderKind::Buy,
                partially_fillable: true,
                ..Default::default()
            },
            metadata: OrderMetadata {
                executed_buy_amount: 9_u32.into(),
                full_fee_amount: 200_u32.into(),
                ..Default::default()
            },
            ..Default::default()
        }
        .into();
        let remaining = Remaining::from_order(&order).unwrap();
        assert_eq!(remaining.remaining(100.into()).unwrap(), 10.into());
        assert_eq!(remaining.remaining(10.into()).unwrap(), 1.into());
        assert_eq!(remaining.remaining(101.into()).unwrap(), 10.into());
        assert_eq!(remaining.remaining(200.into()).unwrap(), 20.into());
    }

    #[test]
    fn remaining_amount_errors() {
        // Partially fillable order overflow when computing fill ratio.
        let order = ModelOrder {
            data: OrderData {
                sell_amount: 1000_u32.into(),
                fee_amount: 337_u32.into(),
                buy_amount: U256::MAX.into(),
                kind: OrderKind::Buy,
                partially_fillable: true,
                ..Default::default()
            },
            ..Default::default()
        }
        .into();
        assert!(Remaining::from_order(&order).is_err());

        // Partially filled order overflowing executed amount.
        let order = ModelOrder {
            data: OrderData {
                buy_amount: U256::MAX.into(),
                kind: OrderKind::Buy,
                partially_fillable: true,
                ..Default::default()
            },
            metadata: OrderMetadata {
                executed_buy_amount: BigUint::from(1_u8) << 256,
                ..Default::default()
            },
            ..Default::default()
        }
        .into();
        assert!(Remaining::from_order(&order).is_ok());

        // Partially filled order that has executed more than its maximum.
        let order = ModelOrder {
            data: OrderData {
                sell_amount: 1_u32.into(),
                kind: OrderKind::Sell,
                partially_fillable: true,
                ..Default::default()
            },
            metadata: OrderMetadata {
                executed_sell_amount_before_fees: 2_u32.into(),
                ..Default::default()
            },
            ..Default::default()
        }
        .into();
        assert!(Remaining::from_order(&order).is_err());

        // Partially fillable order with zero amount.
        let order = ModelOrder {
            data: OrderData {
                sell_amount: 0_u32.into(),
                kind: OrderKind::Sell,
                partially_fillable: true,
                ..Default::default()
            },
            ..Default::default()
        }
        .into();
        assert!(Remaining::from_order(&order).is_err());
    }

    #[test]
    fn scale_order_by_available_balance() {
        // Fill-or-kill orders without balance scale to 0 if there is
        // insufficient balance.
        let order = ModelOrder {
            data: OrderData {
                sell_amount: 1000_u32.into(),
                buy_amount: 2000_u32.into(),
                fee_amount: 337_u32.into(),
                kind: OrderKind::Sell,
                partially_fillable: false,
                ..Default::default()
            },
            ..Default::default()
        }
        .into();
        let remaining = Remaining::from_order_with_balance(&order, order.sell_amount - 1).unwrap();
        assert_eq!(remaining.remaining(1000.into()).unwrap(), 0.into());

        // A partially fillable order that has not been executed at all scales
        // to the available balance.
        let order = ModelOrder {
            data: OrderData {
                sell_amount: 800_u32.into(),
                buy_amount: 2000_u32.into(),
                fee_amount: 200_u32.into(),
                kind: OrderKind::Sell,
                partially_fillable: true,
                ..Default::default()
            },
            ..Default::default()
        }
        .into();
        {
            // More than enough balance for the full order.
            let remaining = Remaining::from_order_with_balance(&order, 5000.into()).unwrap();
            assert_eq!(remaining.remaining(800.into()).unwrap(), 800.into());
        }
        {
            // Not enough balance for the full order.
            let remaining = Remaining::from_order_with_balance(&order, 500.into()).unwrap();
            assert_eq!(remaining.remaining(800.into()).unwrap(), 400.into());
        }

        // A partially fillable order that has has been partially executed scales
        // to the remaining execution and available balance.
        let order = ModelOrder {
            data: OrderData {
                sell_amount: 800_u32.into(),
                buy_amount: 2000_u32.into(),
                fee_amount: 200_u32.into(),
                kind: OrderKind::Sell,
                partially_fillable: true,
                ..Default::default()
            },
            metadata: OrderMetadata {
                executed_sell_amount_before_fees: 400_u32.into(),
                ..Default::default()
            },
            ..Default::default()
        }
        .into();
        {
            // More than enough balance for the full order.
            let remaining = Remaining::from_order_with_balance(&order, 5000.into()).unwrap();
            assert_eq!(remaining.remaining(800.into()).unwrap(), 400.into());
        }
        {
            // Not enough balance for the full order.
            let remaining = Remaining::from_order_with_balance(&order, 250.into()).unwrap();
            assert_eq!(remaining.remaining(800.into()).unwrap(), 200.into());
        }
    }

    #[test]
    fn support_scaling_for_large_orders_with_partial_balance() {
        let order: Order = ModelOrder {
            data: OrderData {
                sell_amount: U256::exp10(30).into(),
                buy_amount: 1_u32.into(),
                kind: OrderKind::Sell,
                partially_fillable: true,
                ..Default::default()
            },
            ..Default::default()
        }
        .into();
        let balance = order.sell_amount - 1;

        // Note that we need to scale because of remaining balance, and that
        // we would overflow with these numbers:
        assert!((order.sell_amount * order.sell_amount)
            .checked_mul(balance)
            .is_none());

        // However, `Remaining` supports these large orders with large partial
        // balances as scaling for remaining execution and available balance are
        // done separately.
        let remaining = Remaining::from_order_with_balance(&order, balance).unwrap();
        assert_eq!(remaining.remaining(order.sell_amount).unwrap(), balance);
    }
}
