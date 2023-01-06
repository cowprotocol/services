use crate::database::{
    orders::{FeeUpdate, LimitOrderQuote},
    Postgres,
};
use anyhow::Result;
use database::orders::SurplusFeeQuoteParameters;
use ethcontract::H160;
use futures::StreamExt;
use model::{
    quote::{OrderQuoteSide, SellAmount},
    DomainSeparator,
};
use number_conversions::big_decimal_to_u256;
use shared::{
    order_quoting::{CalculateQuoteError, OrderQuoting, Quote, QuoteParameters},
    price_estimation::PriceEstimationError,
    signature_validator::SignatureValidating,
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
    pub parallelism: usize,
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
            tracing::trace!(?sleep, "sleeping");
            tokio::time::sleep(sleep).await;
        }
    }

    /// Returns whether it is likely that there is no more work.
    async fn update(&self) -> Result<bool> {
        let parameters = self
            .database
            .order_parameters_with_outdated_fees(self.limit_order_age, self.parallelism)
            .await?;
        futures::stream::iter(&parameters)
            .for_each_concurrent(self.parallelism, |parameters| {
                async move {
                    let quote = self.get_quote(parameters).await;
                    self.update_fee(parameters, &quote).await;
                }
                .instrument(tracing::debug_span!("surplus_fee", ?parameters))
            })
            .await;
        Ok(parameters.len() < self.parallelism)
    }

    /// Handles errors internally.
    async fn get_quote(&self, parameters: &SurplusFeeQuoteParameters) -> Option<Quote> {
        let quote_parameters = QuoteParameters {
            sell_token: H160(parameters.sell_token.0),
            buy_token: H160(parameters.buy_token.0),
            side: OrderQuoteSide::Sell {
                sell_amount: SellAmount::AfterFee {
                    value: big_decimal_to_u256(&parameters.sell_amount).unwrap(),
                },
            },
            // The remaining parameters are only relevant for subsidy computation which is
            // irrelevant for the `surplus_fee`.
            ..Default::default()
        };
        match self.quoter.calculate_quote(quote_parameters).await {
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
                tracing::warn!(?parameters, ?err, "limit order quote error");
                Metrics::get()
                    .update_result
                    .with_label_values(&["get_quote_preventable_failure"])
                    .inc();
                None
            }
            Err(err) => {
                tracing::debug!(?parameters, ?err, "limit order unqoutable");
                Metrics::get()
                    .update_result
                    .with_label_values(&["get_quote_unpreventable_failure"])
                    .inc();
                None
            }
        }
    }

    /// Handles errors internally.
    async fn update_fee(&self, parameters: &SurplusFeeQuoteParameters, quote: &Option<Quote>) {
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
        match self
            .database
            .update_limit_order_fees(parameters, &update)
            .await
        {
            Ok(_) => {
                Metrics::get()
                    .update_result
                    .with_label_values(&["update_fee_ok"])
                    .inc();
            }
            Err(err) => {
                tracing::warn!(?parameters, ?err, "limit order fee update db error");
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
