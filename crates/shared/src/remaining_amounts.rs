use anyhow::{Context, Result};
use model::{auction::Order, order::OrderKind};
use primitive_types::U256;

/// Calculates the remaining amounts for an order.
///
/// For example, when a sell order has half of its sell amount already executed then the remaining
/// buy and fee amounts are also half of their original.
///
/// Works like the smart contract by taking intermediate overflows into account for partially
/// fillable orders.
pub struct Remaining {
    numerator: U256,
    denominator: U256,
}

impl Remaining {
    pub fn from_partially_fillable(total: U256, executed: U256) -> Result<Self> {
        Ok(Self {
            numerator: total
                .checked_sub(executed)
                .context("executed larger than total")?,
            denominator: total,
        })
    }

    pub fn from_fill_or_kill(has_executed: bool) -> Self {
        // fill-or-kill orders do not have their amounts scaled in the contract so using the
        // partially fillable logic would be wrong because it could error in `remaining` when the
        // contract wouldn't.
        Self {
            numerator: if has_executed { 0.into() } else { 1.into() },
            denominator: 1.into(),
        }
    }

    pub fn from_order(order: &Order) -> Result<Self> {
        let total = match order.data.kind {
            OrderKind::Buy => order.data.buy_amount,
            OrderKind::Sell => order.data.sell_amount,
        };
        let executed = order.metadata.executed_amount;
        if order.data.partially_fillable {
            Self::from_partially_fillable(total, executed)
        } else {
            Ok(Self::from_fill_or_kill(!executed.is_zero()))
        }
    }

    /// Returns Err if the contract would error due to intermediate overflow.
    pub fn remaining(&self, total: U256) -> Result<U256> {
        total
            .checked_mul(self.numerator)
            .and_then(|product| product.checked_div(self.denominator))
            .context("overflow")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use model::{auction::OrderMetadata, order::OrderData};

    #[test]
    fn computes_remaining_order_amounts() {
        // For fill-or-kill orders, we don't overflow even for very large buy
        // orders (where `{sell,fee}_amount * buy_amount` would overflow).
        let order = Order {
            data: OrderData {
                sell_amount: 1000.into(),
                buy_amount: U256::MAX,
                fee_amount: 337.into(),
                kind: OrderKind::Buy,
                partially_fillable: false,
                ..Default::default()
            },
            metadata: OrderMetadata {
                full_fee_amount: 42.into(),
                ..Default::default()
            },
            ..Default::default()
        };
        let remaining = Remaining::from_order(&order).unwrap();
        assert_eq!(remaining.remaining(1000.into()).unwrap(), 1000.into());
        assert_eq!(remaining.remaining(U256::MAX).unwrap(), U256::MAX);

        // For partially fillable orders that are untouched, returns the full
        // order amounts.
        let order = Order {
            data: OrderData {
                sell_amount: 10.into(),
                buy_amount: 11.into(),
                fee_amount: 12.into(),
                kind: OrderKind::Sell,
                partially_fillable: true,
                ..Default::default()
            },
            metadata: OrderMetadata {
                executed_amount: 0.into(),
                full_fee_amount: 13.into(),
                ..Default::default()
            },
            ..Default::default()
        };
        let remaining = Remaining::from_order(&order).unwrap();
        assert_eq!(remaining.remaining(10.into()).unwrap(), 10.into());
        assert_eq!(remaining.remaining(13.into()).unwrap(), 13.into());

        // Scales amounts by how much has been executed. Rounds down like the
        // settlement contract.
        let order = Order {
            data: OrderData {
                sell_amount: 100.into(),
                buy_amount: 100.into(),
                fee_amount: 101.into(),
                kind: OrderKind::Sell,
                partially_fillable: true,
                ..Default::default()
            },
            metadata: OrderMetadata {
                executed_amount: 90.into(),
                full_fee_amount: 200.into(),
                ..Default::default()
            },
            ..Default::default()
        };
        let remaining = Remaining::from_order(&order).unwrap();
        assert_eq!(remaining.remaining(100.into()).unwrap(), 10.into());
        assert_eq!(remaining.remaining(101.into()).unwrap(), 10.into());
        assert_eq!(remaining.remaining(200.into()).unwrap(), 20.into());

        let order = Order {
            data: OrderData {
                sell_amount: 100.into(),
                buy_amount: 10.into(),
                fee_amount: 101.into(),
                kind: OrderKind::Buy,
                partially_fillable: true,
                ..Default::default()
            },
            metadata: OrderMetadata {
                executed_amount: 9_u32.into(),
                full_fee_amount: 200.into(),
                ..Default::default()
            },
            ..Default::default()
        };
        let remaining = Remaining::from_order(&order).unwrap();
        assert_eq!(remaining.remaining(100.into()).unwrap(), 10.into());
        assert_eq!(remaining.remaining(10.into()).unwrap(), 1.into());
        assert_eq!(remaining.remaining(101.into()).unwrap(), 10.into());
        assert_eq!(remaining.remaining(200.into()).unwrap(), 20.into());
    }

    #[test]
    fn remaining_amount_errors() {
        // Partially fillable order overflow when computing fill ratio.
        let order = Order {
            data: OrderData {
                sell_amount: 1000.into(),
                fee_amount: 337.into(),
                buy_amount: U256::MAX,
                kind: OrderKind::Buy,
                partially_fillable: true,
                ..Default::default()
            },
            ..Default::default()
        };
        let remaining = Remaining::from_order(&order).unwrap();
        assert!(remaining.remaining(U256::MAX).is_err());

        // Partially filled order that has executed more than its maximum.
        let order = Order {
            data: OrderData {
                sell_amount: 1.into(),
                kind: OrderKind::Sell,
                partially_fillable: true,
                ..Default::default()
            },
            metadata: OrderMetadata {
                executed_amount: 2.into(),
                ..Default::default()
            },
            ..Default::default()
        };
        assert!(Remaining::from_order(&order).is_err());

        // Partially fillable order with zero amount.
        let order = Order {
            data: OrderData {
                sell_amount: 0.into(),
                kind: OrderKind::Sell,
                partially_fillable: true,
                ..Default::default()
            },
            ..Default::default()
        };
        let remaining = Remaining::from_order(&order).unwrap();
        assert!(remaining.remaining(1.into()).is_err());
        assert!(remaining.remaining(1000.into()).is_err());
        assert!(remaining.remaining(U256::MAX).is_err());
    }
}
