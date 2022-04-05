use model::{auction::Auction, order::OrderUid};
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
    pub fn update_and_filter(&mut self, auction: &mut Auction) {
        // If api has seen block X then trades starting at X + 1 are still in flight.
        self.in_flight = self
            .in_flight
            .split_off(&(auction.latest_settlement_block + 1));

        // TODO - could model inflight_trades as HashMap<OrderUid, Vec<Trade>>
        // https://github.com/cowprotocol/services/issues/123
        // Note that this is pessimistaic as it will result in not using the
        // remaining available amount of a partially fillable order while it is
        // in-flight. This is done to avoid `order filled` reverts.
        let in_flight = self
            .in_flight
            .values()
            .flatten()
            .copied()
            .collect::<HashSet<_>>();
        auction
            .orders
            .retain(|order| !in_flight.contains(&order.metadata.uid));
    }

    pub fn mark_settled_orders(&mut self, block: u64, orders: impl Iterator<Item = OrderUid>) {
        self.in_flight.entry(block).or_default().extend(orders);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use model::order::Order;

    #[test]
    fn test() {
        let mut inflight = InFlightOrders::default();
        inflight.mark_settled_orders(1, [OrderUid::from_integer(0)].into_iter());
        let mut order0 = Order::default();
        order0.metadata.uid = OrderUid::from_integer(0);
        let mut order1 = Order::default();
        order1.metadata.uid = OrderUid::from_integer(1);
        let mut auction = Auction {
            block: 0,
            orders: vec![order0, order1],
            ..Default::default()
        };

        let mut update_and_get_filtered_orders = |auction: &Auction| {
            let mut auction = auction.clone();
            inflight.update_and_filter(&mut auction);
            auction.orders
        };

        let filtered = update_and_get_filtered_orders(&auction);
        assert_eq!(filtered.len(), 1);

        auction.block = 1;
        let filtered = update_and_get_filtered_orders(&auction);
        assert_eq!(filtered.len(), 1);

        auction.latest_settlement_block = 1;
        let filtered = update_and_get_filtered_orders(&auction);
        assert_eq!(filtered.len(), 2);
    }
}
