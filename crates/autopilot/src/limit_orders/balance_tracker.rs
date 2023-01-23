use crate::database::Postgres;
use anyhow::Result;
use futures::StreamExt;
use primitive_types::U256;
use prometheus::IntGaugeVec;
use shared::{
    account_balances::{BalanceFetching, Query},
    current_block::{into_stream, CurrentBlockStream},
};
use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
};
use tracing::Instrument;

#[derive(prometheus_metric_storage::MetricStorage)]
pub struct Metrics {
    /// Results of last run loop of the BalanceTracker
    #[metric(labels("result"))]
    limit_orders_balances: IntGaugeVec,
}

/// Updates `has_sufficient_balance` column of open orders.
pub struct BalanceTracker {
    balance_fetcher: Arc<dyn BalanceFetching>,
    database: Postgres,
    metrics: &'static Metrics,
}

fn get_initialized_merics() -> &'static Metrics {
    let metrics = Metrics::instance(global_metrics::get_metric_storage_registry()).unwrap();

    let gauges = &metrics.limit_orders_balances;
    gauges.with_label_values(&["error"]).set(0);
    gauges.with_label_values(&["sufficient"]).set(0);
    gauges.with_label_values(&["insufficient"]).set(0);

    metrics
}

impl BalanceTracker {
    pub fn new(balance_fetcher: Arc<dyn BalanceFetching>, database: Postgres) -> Self {
        Self {
            balance_fetcher,
            database,
            metrics: get_initialized_merics(),
        }
    }

    // Updates the `has_sufficient_balance` flag of open limit orders in the background on every
    // new block.
    pub fn spawn(self, block_stream: CurrentBlockStream) {
        tracing::debug!("spawning background task of BalanceTracker");
        tokio::spawn(
            self.background_task(block_stream)
                .instrument(tracing::info_span!("balance_tracker")),
        );
    }

    async fn background_task(self, block_stream: CurrentBlockStream) {
        let mut block_stream = into_stream(block_stream);
        while block_stream.next().await.is_some() {
            if let Err(err) = self.update_payable_orders().await {
                tracing::error!(?err, "error updating has_sufficient_balance flag, consider disabling --skip-quoting-unfunded-orders");
            }
        }
        tracing::error!("current block stream terminated unexpectedly");
    }

    async fn update_payable_orders(&self) -> Result<()> {
        let orders = self
            .database
            .open_limit_orders_without_pre_interactions()
            .await?;

        // Collect orders in a HashSet first to avoid duplicated requests.
        let queries: HashSet<_> = orders.iter().map(Query::from_order).collect();
        let queries: Vec<_> = queries.into_iter().collect();

        let start = std::time::Instant::now();
        let balances = self.balance_fetcher.get_balances(&queries).await;
        let balances: HashMap<_, _> = queries
            .iter()
            .zip(balances.into_iter())
            .map(|(query, result)| (query, result.unwrap_or_default()))
            .collect();

        let mut errors = 0;
        let mut sufficient = 0;
        let mut insufficient = 0;

        let updates: Vec<_> = orders
            .iter()
            .map(|order| {
                let balance = balances.get(&Query::from_order(order));

                let has_sufficient_balance = if let Some(balance) = balance {
                    let has_sufficient_balance = match order.data.partially_fillable {
                        true => balance >= &U256::one(), // any amount would be enough
                        false => balance >= &order.data.sell_amount,
                    };
                    sufficient += i64::from(has_sufficient_balance);
                    insufficient += i64::from(!has_sufficient_balance);
                    has_sufficient_balance
                } else {
                    errors += 1;
                    // In case the balance couldn't be fetched err on the safe side and assume
                    // the order can be filled to not discard limit orders unjustly.
                    true
                };

                (order.metadata.uid, has_sufficient_balance)
            })
            .collect();

        tracing::debug!(
            orders = orders.len(),
            balances = queries.len(),
            errors,
            time =? start.elapsed(),
            "fetched balances"
        );

        let gauges = &self.metrics.limit_orders_balances;
        gauges.with_label_values(&["error"]).set(errors);
        gauges.with_label_values(&["sufficient"]).set(sufficient);
        gauges
            .with_label_values(&["insufficient"])
            .set(insufficient);

        self.database
            .update_has_sufficient_balance_flags(&updates)
            .await
    }
}
