use model::{
    order::{Order, OrderUid},
    SolvableOrders,
};
use std::collections::{BTreeMap, HashSet};

/// After a settlement transaction we need to keep track of in flight orders until the api has
/// seen the tx. Otherwise we would attempt to solve already matched orders again leading to
/// failures.
#[derive(Default)]
pub struct InFlightOrders {
    /// Maps block to orders settled in that block.
    in_flight: BTreeMap<u64, Vec<OrderUid>>,
}

impl InFlightOrders {
    /// Takes note of the new set of solvable orders and returns the ones that aren't in flight.
    pub fn update_and_filter(&mut self, new: SolvableOrders) -> Vec<Order> {
        // If api has seen block X then trades starting at X + 1 are still in flight.
        self.in_flight = self.in_flight.split_off(&(new.latest_settlement_block + 1));
        let mut orders = new.orders;
        // TODO - could model inflight_trades as HashMap<OrderUid, Vec<Trade>>
        // https://github.com/gnosis/gp-v2-services/issues/673
        // Note that this will result in simulation error "GPv2: order filled" if the
        // next solver run loop tries to match the order again beyond its remaining amount.
        let in_flight = self
            .in_flight
            .values()
            .flatten()
            .copied()
            .collect::<HashSet<_>>();
        orders.retain(|order| {
            order.order_creation.partially_fillable
                || !in_flight.contains(&order.order_meta_data.uid)
        });
        orders
    }

    pub fn mark_settled_orders(&mut self, block: u64, orders: impl Iterator<Item = OrderUid>) {
        self.in_flight.entry(block).or_default().extend(orders);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test() {
        let mut inflight = InFlightOrders::default();
        inflight.mark_settled_orders(1, std::array::IntoIter::new([OrderUid::from_integer(0)]));
        let mut order0 = Order::default();
        order0.order_meta_data.uid = OrderUid::from_integer(0);
        order0.order_creation.partially_fillable = true;
        let mut order1 = Order::default();
        order1.order_meta_data.uid = OrderUid::from_integer(1);
        let mut solvable_orders = SolvableOrders {
            orders: vec![order0, order1],
            latest_settlement_block: 0,
        };

        let filtered = inflight.update_and_filter(solvable_orders.clone());
        assert_eq!(filtered.len(), 2);

        solvable_orders.orders[0].order_creation.partially_fillable = false;
        let filtered = inflight.update_and_filter(solvable_orders.clone());
        assert_eq!(filtered.len(), 1);

        solvable_orders.latest_settlement_block = 1;
        let filtered = inflight.update_and_filter(solvable_orders);
        assert_eq!(filtered.len(), 2);
    }
}
