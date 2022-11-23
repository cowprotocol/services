use chrono::Duration;
use prometheus::IntGauge;

use crate::database::Postgres;

pub struct LimitOrderMetrics {
    pub limit_order_age: Duration,
    pub loop_delay: std::time::Duration,
    pub database: Postgres,
}

impl LimitOrderMetrics {
    pub fn spawn(self) {
        tokio::spawn(async move {
            let limit_orders_gauge = gauge("limit_orders", "Valid limit orders.");
            let quoted_limit_orders_gauge = gauge("quoted_limit_orders", "Quoted limit orders.");
            let unquoted_limit_orders_gauge = gauge(
                "unquoted_limit_orders",
                "Unquoted or outdated limit orders.",
            );
            loop {
                let limit_orders = self.database.count_limit_orders().await.unwrap();
                let unquoted_limit_orders = self
                    .database
                    .count_limit_orders_with_outdated_fees(self.limit_order_age)
                    .await
                    .unwrap();
                let quoted_limit_orders = limit_orders - unquoted_limit_orders;
                limit_orders_gauge.set(limit_orders);
                unquoted_limit_orders_gauge.set(unquoted_limit_orders);
                quoted_limit_orders_gauge.set(quoted_limit_orders);
                tokio::time::sleep(self.loop_delay).await;
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
