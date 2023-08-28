use {crate::database::Postgres, prometheus::IntGauge, std::time::Duration, tracing::Instrument};

pub struct LimitOrderMetrics {
    /// At that age the [`LimitOrderQuoter`] would update the `surplus_fee`.
    pub quoting_age: chrono::Duration,
    /// At that age the [`SolvableOrdersCache`] would consider a `surplus_fee`
    /// too old.
    pub validity_age: chrono::Duration,
    pub database: Postgres,
}

#[derive(prometheus_metric_storage::MetricStorage)]
pub struct Metrics {
    /// Open limit orders
    limit_orders: IntGauge,

    /// Quoted limit orders.
    quoted_limit_orders: IntGauge,

    /// Limit orders awaiting quote.
    unquoted_limit_orders: IntGauge,

    /// Limit orders usable in the auction.
    usable_limit_orders: IntGauge,

    /// Limit orders with surplus_fee too old for the auction.
    unusable_limit_orders: IntGauge,
}

impl LimitOrderMetrics {
    pub fn spawn(self) {
        tokio::spawn(
            async move {
                let metrics = Metrics::instance(observe::metrics::get_storage_registry()).unwrap();

                loop {
                    let limit_orders = self.database.count_fok_limit_orders().await.unwrap();
                    let awaiting_quote = self
                        .database
                        .count_limit_orders_with_outdated_fees(self.quoting_age)
                        .await
                        .unwrap();
                    let unusable = self
                        .database
                        .count_limit_orders_with_outdated_fees(self.validity_age)
                        .await
                        .unwrap();

                    let quoted_limit_orders = limit_orders - awaiting_quote;
                    let usable_limit_orders = limit_orders - unusable;

                    metrics.limit_orders.set(limit_orders);
                    metrics.unquoted_limit_orders.set(awaiting_quote);
                    metrics.quoted_limit_orders.set(quoted_limit_orders);
                    metrics.usable_limit_orders.set(usable_limit_orders);
                    metrics.unusable_limit_orders.set(unusable);

                    tokio::time::sleep(Duration::from_secs(10)).await;
                }
            }
            .instrument(tracing::info_span!("limit_order_metrics")),
        );
    }
}
