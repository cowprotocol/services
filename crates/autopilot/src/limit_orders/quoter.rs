use crate::database::{orders::FeeUpdate, Postgres};
use anyhow::Result;
use chrono::{Duration, Utc};
use futures::future::join_all;
use model::{
    order::{Order, OrderKind},
    quote::{OrderQuoteSide, QuoteSigningScheme, SellAmount},
    signature::{hashed_eip712_message, Signature},
    DomainSeparator,
};
use shared::{
    order_quoting::{OrderQuoting, QuoteParameters},
    signature_validator::{SignatureCheck, SignatureValidating},
};
use std::sync::Arc;

/// Background task which quotes all limit orders and sets the surplus_fee for each one
/// to the fee returned by the quoting process. If quoting fails, the corresponding
/// order is skipped.
pub struct LimitOrderQuoter {
    pub limit_order_age: Duration,
    pub loop_delay: std::time::Duration,
    pub quoter: Arc<dyn OrderQuoting>,
    pub database: Postgres,
    pub signature_validator: Arc<dyn SignatureValidating>,
    pub domain_separator: DomainSeparator,
}

impl LimitOrderQuoter {
    pub fn spawn(self) {
        tokio::spawn(async move {
            loop {
                if let Err(err) = self.update().await {
                    tracing::error!(?err, "failed to update limit order surplus");
                }
                tokio::time::sleep(self.loop_delay).await;
            }
        });
    }

    async fn update(&self) -> Result<()> {
        let mut failed_orders = 0;
        loop {
            let orders = self
                .database
                .limit_orders_with_outdated_fees(self.limit_order_age)
                .await?;
            if orders.is_empty() {
                break;
            }
            for chunk in orders.chunks(10) {
                failed_orders += join_all(chunk.iter().map(|order| async move {
                    let update_result = self.update_surplus_fee(order).await;
                    if let Err(err) = &update_result {
                        let order_uid = &order.metadata.uid;
                        tracing::warn!(%order_uid, ?err, "skipped limit order due to error");
                    };
                    usize::from(update_result.is_err())
                }))
                .await
                .into_iter()
                .sum::<usize>();
            }
        }
        Metrics::on_failed(failed_orders.try_into().unwrap());
        Ok(())
    }

    async fn update_surplus_fee(&self, order: &Order) -> Result<()> {
        let mut quote = self
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
                signing_scheme: match &order.signature {
                    Signature::Eip712(_) => QuoteSigningScheme::Eip712,
                    Signature::EthSign(_) => QuoteSigningScheme::EthSign,
                    Signature::Eip1271(signature) => {
                        let additional_gas = self
                            .signature_validator
                            .validate_signature_and_get_additional_gas(SignatureCheck {
                                signer: order.metadata.owner,
                                hash: hashed_eip712_message(
                                    &self.domain_separator,
                                    &order.data.hash_struct(),
                                ),
                                signature: signature.to_owned(),
                            })
                            .await?;
                        QuoteSigningScheme::Eip1271 {
                            onchain_order: false,
                            verification_gas_limit: additional_gas,
                        }
                    }
                    Signature::PreSign => QuoteSigningScheme::PreSign {
                        onchain_order: false,
                    },
                },
            })
            .await?;
        self.database
            .update_limit_order_fees(
                &order.metadata.uid,
                &FeeUpdate {
                    surplus_fee: quote.fee_amount,
                    full_fee_amount: quote.full_fee_amount,
                },
            )
            .await?;
        // Make quote last long enough to compute risk adjusted rewards for the order.
        quote.data.expiration = Utc::now() + self.limit_order_age;
        self.quoter.store_quote(quote).await?;
        Ok(())
    }
}

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
