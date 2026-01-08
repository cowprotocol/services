use {
    alloy::primitives::U256,
    anyhow::{Context, Result},
    model::order::{Order as ModelOrder, OrderKind},
    number::{conversions::big_uint_to_u256, ratio_ext::RatioExt},
};

type Ratio = num::rational::Ratio<U256>;

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
    execution: Ratio,

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
    balance: Ratio,
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

            let sell_amount = execution
                .scalar_mul(order.sell_amount)
                .context("overflow scaling sell amount for execution")?;
            let fee_amount = execution
                .scalar_mul(order.fee_amount)
                .context("overflow scaling fee amount for execution")?;

            let need = sell_amount
                .checked_add(fee_amount)
                .context("partially fillable need calculation overflow")?;

            let balance = if sell_balance < need {
                Ratio::new_raw(sell_balance, need)
            } else {
                Ratio::ONE
            };

            Ok(Self { execution, balance })
        } else {
            let sell = order
                .sell_amount
                .checked_add(order.fee_amount)
                .context("overflow sell + fee amount")?;

            let execution = if order.executed_amount.is_zero() {
                Ratio::ONE
            } else {
                Ratio::ZERO
            };

            let balance = if sell_balance >= sell {
                Ratio::ONE
            } else {
                Ratio::ZERO
            };

            Ok(Self { execution, balance })
        }
    }

    /// Returns Err if the contract would error due to intermediate overflow.
    pub fn remaining(&self, total: U256) -> Result<U256> {
        let order_execution = self
            .execution
            .scalar_mul(total)
            .context("overflow scaling for order execution")?;
        self.balance
            .full_scalar_mul(order_execution)
            .context("overflow scaling for available balance")
    }
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
            buy_amount: o.data.buy_amount,
            sell_amount: o.data.sell_amount,
            fee_amount: o.data.fee_amount,
            executed_amount: match o.data.kind {
                // A real buy order cannot execute more than U256::MAX so in order to make this
                // function infallible we treat a larger amount as a full execution.
                OrderKind::Buy => {
                    big_uint_to_u256(&o.metadata.executed_buy_amount).unwrap_or(o.data.buy_amount)
                }
                OrderKind::Sell => o.metadata.executed_sell_amount_before_fees,
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
                sell_amount: alloy::primitives::U256::from(1000),
                buy_amount: alloy::primitives::U256::MAX,
                fee_amount: alloy::primitives::U256::from(337),
                kind: OrderKind::Buy,
                partially_fillable: false,
                ..Default::default()
            },
            ..Default::default()
        }
        .into();
        let remaining = Remaining::from_order(&order).unwrap();
        assert_eq!(
            remaining.remaining(U256::from(1000)).unwrap(),
            U256::from(1000)
        );
        assert_eq!(remaining.remaining(U256::MAX).unwrap(), U256::MAX);

        // For partially fillable orders that are untouched, returns the full
        // order amounts.
        let order = ModelOrder {
            data: OrderData {
                sell_amount: alloy::primitives::U256::from(10),
                buy_amount: alloy::primitives::U256::from(11),
                fee_amount: alloy::primitives::U256::from(12),
                kind: OrderKind::Sell,
                partially_fillable: true,
                ..Default::default()
            },
            metadata: OrderMetadata {
                executed_sell_amount_before_fees: U256::ZERO,
                ..Default::default()
            },
            ..Default::default()
        }
        .into();
        let remaining = Remaining::from_order(&order).unwrap();
        assert_eq!(remaining.remaining(U256::from(10)).unwrap(), U256::from(10));
        assert_eq!(remaining.remaining(U256::from(13)).unwrap(), U256::from(13));

        // Scales amounts by how much has been executed. Rounds down like the
        // settlement contract.
        let order = ModelOrder {
            data: OrderData {
                sell_amount: alloy::primitives::U256::from(100),
                buy_amount: alloy::primitives::U256::from(100),
                fee_amount: alloy::primitives::U256::from(101),
                kind: OrderKind::Sell,
                partially_fillable: true,
                ..Default::default()
            },
            metadata: OrderMetadata {
                executed_sell_amount_before_fees: U256::from(90),
                ..Default::default()
            },
            ..Default::default()
        }
        .into();
        let remaining = Remaining::from_order(&order).unwrap();
        assert_eq!(
            remaining.remaining(U256::from(100)).unwrap(),
            U256::from(10)
        );
        assert_eq!(
            remaining.remaining(U256::from(101)).unwrap(),
            U256::from(10)
        );
        assert_eq!(
            remaining.remaining(U256::from(200)).unwrap(),
            U256::from(20)
        );

        let order = ModelOrder {
            data: OrderData {
                sell_amount: alloy::primitives::U256::from(100),
                buy_amount: alloy::primitives::U256::from(10),
                fee_amount: alloy::primitives::U256::from(101),
                kind: OrderKind::Buy,
                partially_fillable: true,
                ..Default::default()
            },
            metadata: OrderMetadata {
                executed_buy_amount: 9_u32.into(),
                ..Default::default()
            },
            ..Default::default()
        }
        .into();
        let remaining = Remaining::from_order(&order).unwrap();
        assert_eq!(
            remaining.remaining(U256::from(100)).unwrap(),
            U256::from(10)
        );
        assert_eq!(remaining.remaining(U256::from(10)).unwrap(), U256::ONE);
        assert_eq!(
            remaining.remaining(U256::from(101)).unwrap(),
            U256::from(10)
        );
        assert_eq!(
            remaining.remaining(U256::from(200)).unwrap(),
            U256::from(20)
        );
    }

    #[test]
    fn remaining_amount_errors() {
        // Partially fillable order overflow when computing fill ratio.
        let order = ModelOrder {
            data: OrderData {
                sell_amount: alloy::primitives::U256::from(1000),
                fee_amount: alloy::primitives::U256::from(337),
                buy_amount: alloy::primitives::U256::MAX,
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
                buy_amount: alloy::primitives::U256::MAX,
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
                sell_amount: alloy::primitives::U256::ONE,
                kind: OrderKind::Sell,
                partially_fillable: true,
                ..Default::default()
            },
            metadata: OrderMetadata {
                executed_sell_amount_before_fees: U256::from(2),
                ..Default::default()
            },
            ..Default::default()
        }
        .into();
        assert!(Remaining::from_order(&order).is_err());

        // Partially fillable order with zero amount.
        let order = ModelOrder {
            data: OrderData {
                sell_amount: alloy::primitives::U256::ZERO,
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
                sell_amount: alloy::primitives::U256::from(1000),
                buy_amount: alloy::primitives::U256::from(2000),
                fee_amount: alloy::primitives::U256::from(337),
                kind: OrderKind::Sell,
                partially_fillable: false,
                ..Default::default()
            },
            ..Default::default()
        }
        .into();
        let remaining =
            Remaining::from_order_with_balance(&order, order.sell_amount - U256::ONE).unwrap();
        assert!(remaining.remaining(U256::from(1000)).unwrap().is_zero());

        // A partially fillable order that has not been executed at all scales
        // to the available balance.
        let order = ModelOrder {
            data: OrderData {
                sell_amount: alloy::primitives::U256::from(800),
                buy_amount: alloy::primitives::U256::from(2000),
                fee_amount: alloy::primitives::U256::from(200),
                kind: OrderKind::Sell,
                partially_fillable: true,
                ..Default::default()
            },
            ..Default::default()
        }
        .into();
        {
            // More than enough balance for the full order.
            let remaining = Remaining::from_order_with_balance(&order, U256::from(5000)).unwrap();
            assert_eq!(
                remaining.remaining(U256::from(800)).unwrap(),
                U256::from(800)
            );
        }
        {
            // Not enough balance for the full order.
            let remaining = Remaining::from_order_with_balance(&order, U256::from(500)).unwrap();
            assert_eq!(
                remaining.remaining(U256::from(800)).unwrap(),
                U256::from(400)
            );
        }

        // A partially fillable order that has has been partially executed scales
        // to the remaining execution and available balance.
        let order = ModelOrder {
            data: OrderData {
                sell_amount: U256::from(800),
                buy_amount: U256::from(2000),
                fee_amount: U256::from(200),
                kind: OrderKind::Sell,
                partially_fillable: true,
                ..Default::default()
            },
            metadata: OrderMetadata {
                executed_sell_amount_before_fees: U256::from(400),
                ..Default::default()
            },
            ..Default::default()
        }
        .into();
        {
            // More than enough balance for the full order.
            let remaining = Remaining::from_order_with_balance(&order, U256::from(5000)).unwrap();
            assert_eq!(
                remaining.remaining(U256::from(800)).unwrap(),
                U256::from(400)
            );
        }
        {
            // Not enough balance for the full order.
            let remaining = Remaining::from_order_with_balance(&order, U256::from(250)).unwrap();
            assert_eq!(
                remaining.remaining(U256::from(800)).unwrap(),
                U256::from(200)
            );
        }
    }

    #[test]
    fn support_scaling_for_large_orders_with_partial_balance() {
        let order: Order = ModelOrder {
            data: OrderData {
                sell_amount: U256::from(10).pow(U256::from(30)),
                buy_amount: U256::ONE,
                kind: OrderKind::Sell,
                partially_fillable: true,
                ..Default::default()
            },
            ..Default::default()
        }
        .into();
        let balance = order.sell_amount - U256::ONE;

        // Note that we need to scale because of remaining balance, and that
        // we would overflow with these numbers:
        assert!(
            (order.sell_amount * order.sell_amount)
                .checked_mul(balance)
                .is_none()
        );

        // However, `Remaining` supports these large orders with large partial
        // balances as scaling for remaining execution and available balance are
        // done separately.
        let remaining = Remaining::from_order_with_balance(&order, balance).unwrap();
        assert_eq!(remaining.remaining(order.sell_amount).unwrap(), balance);
    }
}
