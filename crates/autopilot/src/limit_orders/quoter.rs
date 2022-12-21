use crate::database::{
    orders::{FeeUpdate, LimitOrderQuote},
    Postgres,
};
use anyhow::Result;
use futures::StreamExt;
use model::{
    order::{Order, OrderKind, OrderUid},
    quote::{OrderQuoteSide, QuoteSigningScheme, SellAmount},
    signature::{hashed_eip712_message, Signature},
    DomainSeparator,
};
use shared::{
    order_quoting::{CalculateQuoteError, OrderQuoting, Quote, QuoteParameters},
    price_estimation::PriceEstimationError,
    signature_validator::{SignatureCheck, SignatureValidating, SignatureValidationError},
};
use std::{sync::Arc, time::Duration};
use tracing::Instrument as _;

/// Background task which quotes all limit orders and sets the surplus_fee for each one
/// to the fee returned by the quoting process. If quoting fails, the corresponding
/// order is skipped.
pub struct LimitOrderQuoter {
    pub limit_order_age: chrono::Duration,
    pub quoter: Arc<dyn OrderQuoting>,
    pub database: Postgres,
    pub signature_validator: Arc<dyn SignatureValidating>,
    pub domain_separator: DomainSeparator,
}

impl LimitOrderQuoter {
    pub fn spawn(self) {
        tokio::spawn(async move { self.background_task().await });
    }

    async fn background_task(&self) -> ! {
        loop {
            let sleep = match self.update().await {
                // Prevent busy looping on the database if there is no work to be done.
                Ok(true) => Duration::from_secs_f32(10.),
                Ok(false) => Duration::from_secs_f32(0.),
                Err(err) => {
                    tracing::error!(?err, "limit order quoter update error");
                    Duration::from_secs_f32(1.)
                }
            };
            tokio::time::sleep(sleep).await;
        }
    }

    /// Returns whether it is likely that there is no more work.
    async fn update(&self) -> Result<bool> {
        const PARALLELISM: usize = 5;
        let orders = self
            .database
            .limit_orders_with_outdated_fees(self.limit_order_age, PARALLELISM)
            .await?;
        futures::stream::iter(&orders)
            .for_each_concurrent(PARALLELISM, |order| {
                async move {
                    let quote = self.get_quote(order).await;
                    self.update_fee(&order.metadata.uid, &quote).await;
                }
                .instrument(tracing::debug_span!(
                    "surplus_fee",
                    order =% order.metadata.uid
                ))
            })
            .await;
        Ok(orders.len() < PARALLELISM)
    }

    /// Handles errors internally.
    async fn get_quote(&self, order: &Order) -> Option<Quote> {
        let uid = &order.metadata.uid;
        let signing_scheme = match &order.signature {
            Signature::Eip712(_) => QuoteSigningScheme::Eip712,
            Signature::EthSign(_) => QuoteSigningScheme::EthSign,
            Signature::Eip1271(signature) => {
                let additional_gas = match self
                    .signature_validator
                    .validate_signature_and_get_additional_gas(SignatureCheck {
                        signer: order.metadata.owner,
                        hash: hashed_eip712_message(
                            &self.domain_separator,
                            &order.data.hash_struct(),
                        ),
                        signature: signature.to_owned(),
                    })
                    .await
                {
                    Ok(gas) => gas,
                    Err(SignatureValidationError::Invalid) => {
                        tracing::debug!(%uid, "limit order has an invalid signature");
                        Metrics::get()
                            .update_result
                            .with_label_values(&["get_quote_unpreventable_failure"])
                            .inc();
                        return None;
                    }
                    Err(SignatureValidationError::Other(err)) => {
                        tracing::warn!(%uid, ?err, "limit order signature validation error");
                        Metrics::get()
                            .update_result
                            .with_label_values(&["get_quote_preventable_failure"])
                            .inc();
                        return None;
                    }
                };
                QuoteSigningScheme::Eip1271 {
                    onchain_order: false,
                    verification_gas_limit: additional_gas,
                }
            }
            Signature::PreSign => QuoteSigningScheme::PreSign {
                onchain_order: false,
            },
        };
        let parameters = QuoteParameters {
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
            signing_scheme,
        };
        match self.quoter.calculate_quote(parameters).await {
            Ok(quote) => {
                Metrics::get()
                    .update_result
                    .with_label_values(&["get_quote_ok"])
                    .inc();
                Some(quote)
            }
            Err(
                CalculateQuoteError::Other(err)
                | CalculateQuoteError::Price(PriceEstimationError::Other(err)),
            ) => {
                tracing::warn!(%uid, ?err, "limit order quote error");
                Metrics::get()
                    .update_result
                    .with_label_values(&["get_quote_preventable_failure"])
                    .inc();
                None
            }
            Err(err) => {
                tracing::debug!(%uid, ?err, "limit order unqoutable");
                Metrics::get()
                    .update_result
                    .with_label_values(&["get_quote_unpreventable_failure"])
                    .inc();
                None
            }
        }
    }

    /// Handles errors internally.
    async fn update_fee(&self, uid: &OrderUid, quote: &Option<Quote>) {
        let timestamp = chrono::Utc::now();
        let update = match quote {
            Some(quote) => FeeUpdate::Success {
                timestamp,
                surplus_fee: quote.fee_amount,
                full_fee_amount: quote.full_fee_amount,
                quote: LimitOrderQuote {
                    fee_parameters: quote.data.fee_parameters,
                    sell_amount: quote.sell_amount,
                    buy_amount: quote.buy_amount,
                },
            },
            None => FeeUpdate::Failure { timestamp },
        };
        match self.database.update_limit_order_fees(uid, &update).await {
            Ok(_) => {
                Metrics::get()
                    .update_result
                    .with_label_values(&["update_fee_ok"])
                    .inc();
            }
            Err(err) => {
                tracing::warn!(%uid, ?err, "limit order fee update db error");
                Metrics::get()
                    .update_result
                    .with_label_values(&["update_fee_preventable_failure"])
                    .inc();
            }
        }
    }
}

#[derive(prometheus_metric_storage::MetricStorage, Clone, Debug)]
#[metric(subsystem = "limit_order_quoter")]
struct Metrics {
    /// Categorizes order quote update results.
    #[metric(labels("type"))]
    update_result: prometheus::IntCounterVec,
}

impl Metrics {
    fn get() -> &'static Self {
        Self::instance(global_metrics::get_metric_storage_registry()).unwrap()
    }
}
