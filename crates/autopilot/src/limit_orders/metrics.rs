use std::time::Duration;

use prometheus::IntGauge;

use crate::database::Postgres;

pub struct LimitOrderMetrics {
    /// At that age the [`LimitOrderQuoter`] would update the `surplus_fee`.
    pub quoting_age: chrono::Duration,
    /// At that age the [`SolvableOrdersCache`] would consider a `surplus_fee` too old.
    pub validity_age: chrono::Duration,
    pub database: Postgres,
}

impl LimitOrderMetrics {
    pub fn spawn(self) {
        tokio::spawn(async move {
            let limit_orders_gauge = gauge("limit_orders", "Open limit orders.");
            let quoted_limit_orders_gauge = gauge("quoted_limit_orders", "Quoted limit orders.");
            let unquoted_limit_orders_gauge =
                gauge("unquoted_limit_orders", "Limit orders awaiting a quote.");
            let usable_limit_orders_gauge =
                gauge("usable_limit_orders", "Limit orders usable in the auction.");
            let unusable_limit_orders_gauge = gauge(
                "unusable_limit_orders",
                "Limit orders with surplus_fee too old for the auction.",
            );

            loop {
                let limit_orders = self.database.count_limit_orders().await.unwrap();
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

                limit_orders_gauge.set(limit_orders);
                unquoted_limit_orders_gauge.set(awaiting_quote);
                quoted_limit_orders_gauge.set(quoted_limit_orders);
                usable_limit_orders_gauge.set(usable_limit_orders);
                unusable_limit_orders_gauge.set(unusable);

                tokio::time::sleep(Duration::from_secs(10)).await;
            }
        });
    }
}

fn gauge(name: &str, help: &str) -> IntGauge {
    let registry = global_metrics::get_metrics_registry();
    let gauge = IntGauge::new(name, help).unwrap();
    registry.register(Box::new(gauge.clone())).unwrap();
    gauge
}
