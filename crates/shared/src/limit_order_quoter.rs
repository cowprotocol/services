use crate::order_quoting::{OrderQuoting, QuoteParameters};
use anyhow::Result;
use async_trait::async_trait;
use chrono::Duration;
use ethcontract::U256;
use futures::future::join_all;
use model::{
    order::{Order, OrderKind},
    quote::{default_verification_gas_limit, OrderQuoteSide, QuoteSigningScheme, SellAmount},
    signature::Signature,
};
use std::sync::Arc;

#[derive(prometheus_metric_storage::MetricStorage, Clone, Debug)]
#[metric(subsystem = "limit_order_quoter")]
struct Metrics {
    /// Counter for failed limit orders.
    failed: prometheus::IntCounter,
}

impl Metrics {
    fn on_failed(failed: u64) {
        Self::instance(global_metrics::get_metric_storage_registry())
            .unwrap()
            .failed
            .inc_by(failed);
    }
}

#[derive(Debug)]
pub struct BackgroundConfig {
    pub limit_order_age: Duration,
    pub loop_delay: std::time::Duration,
}

pub fn start_background_quoter(quoter: LimitOrderQuoter, config: BackgroundConfig) {
    tokio::spawn(async move {
        loop {
            if let Err(err) = quoter.quote_outdated_orders(config.limit_order_age).await {
                tracing::error!(?err, "failed to update limit order surplus");
            }
            tokio::time::sleep(config.loop_delay).await;
        }
    });
}

#[mockall::automock]
#[async_trait]
pub trait Storing: Send + Sync {
    /// Updates a surplus fee.
    async fn update_surplus_fee(&self, order: &Order, surplus_fee: U256) -> Result<()>;

    /// Fetches limit orders with surplus fees older than the given duration.
    async fn limit_orders_with_outdated_fees(&self, age: Duration) -> Result<Vec<Order>>;
}

/// Background task which quotes all limit orders and sets the surplus_fee for each one
/// to the fee returned by the quoting process. If quoting fails, the corresponding
/// order is skipped.
#[derive(Clone)]
pub struct LimitOrderQuoter {
    quoter: Arc<dyn OrderQuoting>,
    storage: Arc<dyn Storing>,
}

impl LimitOrderQuoter {
    pub fn new(quoter: Arc<dyn OrderQuoting>, storage: Arc<dyn Storing>) -> Self {
        Self { quoter, storage }
    }

    pub async fn quote(&self, order: &Order) -> Result<()> {
        let quote = self
            .quoter
            .calculate_quote(QuoteParameters {
                sell_token: order.data.sell_token,
                buy_token: order.data.buy_token,
                side: match order.data.kind {
                    OrderKind::Buy => OrderQuoteSide::Buy {
                        buy_amount_after_fee: order.data.buy_amount,
                    },
                    OrderKind::Sell => OrderQuoteSide::Sell {
                        sell_amount: SellAmount::BeforeFee {
                            value: order.data.sell_amount + order.data.fee_amount,
                        },
                    },
                },
                from: order.metadata.owner,
                app_data: order.data.app_data,
                signing_scheme: match order.signature {
                    Signature::Eip712(_) => QuoteSigningScheme::Eip712,
                    Signature::EthSign(_) => QuoteSigningScheme::EthSign,
                    Signature::Eip1271(_) => QuoteSigningScheme::Eip1271 {
                        onchain_order: false,
                        verification_gas_limit: default_verification_gas_limit(),
                    },
                    Signature::PreSign => QuoteSigningScheme::PreSign {
                        onchain_order: false,
                    },
                },
            })
            .await?;
        self.storage
            .update_surplus_fee(order, quote.fee_amount)
            .await
    }

    async fn quote_outdated_orders(&self, limit_order_age: Duration) -> Result<()> {
        let mut failed_orders = 0;
        loop {
            let orders = self
                .storage
                .limit_orders_with_outdated_fees(limit_order_age)
                .await?;
            if orders.is_empty() {
                break;
            }
            for chunk in orders.chunks(10) {
                failed_orders += join_all(chunk.iter().map(|order| async move {
                    match self.quote(order).await {
                        Ok(()) => false,
                        Err(err) => {
                            tracing::warn!(
                                ?err, order_uid = %order.metadata.uid,
                                "failed to quote limit order, skipping"
                            );
                            true
                        }
                    }
                }))
                .await
                .into_iter()
                .filter(|&v| v)
                .count();
            }
        }
        Metrics::on_failed(failed_orders.try_into().unwrap());
        Ok(())
    }
}
