use super::OnchainOrderData;
use database::{
    events::EventIndex, onchain_broadcasted_orders::OnchainOrderPlacement, orders::OrderClass,
};
use itertools::Itertools;
use model::order::OrderUid;
use shared::{
    current_block::RangeInclusive,
    event_handling::MAX_REORG_BLOCK_COUNT,
    orderbook_metrics::{db_order_class_label, operation_label, OrderOperation},
};
use std::collections::{HashMap, HashSet};
use strum::IntoEnumIterator;

#[derive(prometheus_metric_storage::MetricStorage, Clone, Debug)]
#[metric(subsystem = "onchain_orders")]
pub struct Metrics {
    /// Keeps track of errors in picking up onchain orders.
    /// Note that an order might be created even if an error is encountered.
    #[metric(labels("error_type"))]
    onchain_order_errors: prometheus::IntCounterVec,

    /// Counts the number of onchain orders that were created or cancelled.
    /// It accounts for reorg, which means that its value might decrease.
    /// It might provide incorrect data if a reorg happens during a restart of
    /// the services.
    #[metric(labels("kind", "operation"))]
    orders: prometheus::IntGaugeVec,
}

impl Metrics {
    pub fn get() -> &'static Self {
        Self::instance(global_metrics::get_metric_storage_registry())
            .expect("unexpected error getting metrics instance")
    }

    pub fn init(&self) {
        for (class, op) in OrderClass::iter().cartesian_product(OrderOperation::iter()) {
            self.change_orders_by(0, &class, &op);
        }

        // Note: `onchain_order_errors` isn't initialized because keeping track
        // of all possible error labels would require more maintenance than
        // it's worth it.
    }

    pub fn inc_onchain_order_errors(&self, error_label: &str) {
        self.onchain_order_errors
            .with_label_values(&[error_label])
            .inc();
    }

    pub fn change_orders_by(&self, amount: i64, class: &OrderClass, op: &OrderOperation) {
        let class = db_order_class_label(class);
        let op = operation_label(op);
        self.orders.with_label_values(&[class, op]).add(amount);
    }
}

type OrderDetailsForMetrics = (u64, OrderClass, OrderOperation);
pub type OrderDetailsToOrderUids = HashMap<OrderDetailsForMetrics, HashSet<OrderUid>>;
#[derive(Debug, Default)]
pub struct OrdersByBlockNumber(OrderDetailsToOrderUids);

impl OrdersByBlockNumber {
    pub fn prepare_created_orders<W>(orders: &[OnchainOrderData<W>]) -> OrderDetailsToOrderUids {
        let mut to_add = HashMap::<OrderDetailsForMetrics, HashSet<OrderUid>>::new();
        for order in orders {
            to_add
                .entry((
                    order
                        .broadcasted_order_data
                        .0
                        .block_number
                        .try_into()
                        .expect("Block number should not be negative"),
                    order.order.class,
                    OrderOperation::Created,
                ))
                .or_default()
                .insert(OrderUid(order.broadcasted_order_data.1.order_uid.0));
        }

        to_add
    }

    pub fn add_orders(&mut self, to_add: OrderDetailsToOrderUids, metrics: &'static Metrics) {
        for (order_details, order_uids) in to_add {
            let num_changes = order_uids
                .into_iter()
                .map(|uid| self.0.entry(order_details).or_default().insert(uid))
                .filter(|&result| result)
                .count();

            metrics.change_orders_by(num_changes as i64, &order_details.1, &order_details.2);
        }
    }

    pub fn remove_orders_in_range(
        &mut self,
        range: RangeInclusive<u64>,
        metrics: &'static Metrics,
    ) {
        self.0.retain(|(block_nr, class, op), uids| {
            let should_skip = range.contains(*block_nr);
            if should_skip {
                metrics.change_orders_by(-(uids.len() as i64), class, op);
            }
            !should_skip
        });
    }

    pub fn purge_old_orders(&mut self, current_block: u64) {
        self.0
            .retain(|(block_nr, _, _), _| *block_nr < current_block + MAX_REORG_BLOCK_COUNT);
    }
}

pub fn get_most_recent_block(
    invalided_order_uids: &[(EventIndex, database::OrderUid)],
    broadcasted_order_data: &[(database::events::EventIndex, OnchainOrderPlacement)],
) -> Option<u64> {
    [
        invalided_order_uids
            .iter()
            .map(|event| event.0.block_number)
            .max()
            .into_iter(),
        broadcasted_order_data
            .iter()
            .map(|event| event.0.block_number)
            .max()
            .into_iter(),
    ]
    .into_iter()
    .flatten()
    .max()
    .map(|max| max.try_into().expect("Block number should be positive"))
}
