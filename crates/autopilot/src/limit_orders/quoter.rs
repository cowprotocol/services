use crate::database::{
    orders::{FeeUpdate, LimitOrderQuote},
    Postgres,
};
use anyhow::Result;
use database::orders::{OrderFeeSpecifier, OrderQuotingData};
use ethcontract::H160;
use futures::StreamExt;
use itertools::Itertools;
use model::quote::{OrderQuoteSide, SellAmount};
use number_conversions::big_decimal_to_u256;
use primitive_types::U256;
use shared::{
    account_balances::{BalanceFetching, Query},
    db_order_conversions::sell_token_source_from,
    order_quoting::{CalculateQuoteError, OrderQuoting, Quote, QuoteParameters},
    price_estimation::PriceEstimationError,
};
use std::{collections::HashMap, sync::Arc, time::Duration};
use tracing::Instrument as _;

/// Background task which quotes all limit orders and sets the surplus_fee for each one
/// to the fee returned by the quoting process. If quoting fails, the corresponding
/// order is skipped.
pub struct LimitOrderQuoter {
    pub limit_order_age: chrono::Duration,
    pub quoter: Arc<dyn OrderQuoting>,
    pub database: Postgres,
    pub parallelism: usize,
    pub strategies: Vec<QuotingStrategy>,
    pub balance_fetcher: Arc<dyn BalanceFetching>,
    pub batch_size: usize,
}

impl LimitOrderQuoter {
    pub fn spawn(self) {
        tokio::spawn(async move {
            self.background_task()
                .instrument(tracing::info_span!("limit_order_quoter"))
                .await
        });
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
        let orders = self
            .database
            .open_limit_orders(self.limit_order_age)
            .await?;

        let orders = match self.strategies.contains(&QuotingStrategy::SkipUnfunded) {
            true => orders_with_sufficient_balance(&*self.balance_fetcher, orders).await,
            false => orders,
        };

        let order_specs = orders
            .into_iter()
            .map(order_spec_from)
            .unique()
            .take(self.batch_size)
            .collect_vec();

        futures::stream::iter(&order_specs)
            .for_each_concurrent(self.parallelism, |order_spec| {
                async move {
                    let quote = self.get_quote(order_spec).await;
                    self.update_fee(order_spec, &quote).await;
                }
                .instrument(tracing::debug_span!("surplus_fee", ?order_spec))
            })
            .await;
        Ok(order_specs.len() < self.batch_size)
    }

    /// Handles errors internally.
    async fn get_quote(&self, order_spec: &OrderFeeSpecifier) -> Option<Quote> {
        let parameters = QuoteParameters {
            sell_token: H160(order_spec.sell_token.0),
            buy_token: H160(order_spec.buy_token.0),
            side: OrderQuoteSide::Sell {
                sell_amount: SellAmount::AfterFee {
                    value: big_decimal_to_u256(&order_spec.sell_amount).unwrap(),
                },
            },
            // The remaining parameters are only relevant for subsidy computation which is
            // irrelevant for the `surplus_fee`.
            ..Default::default()
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
                tracing::warn!(?order_spec, ?err, "limit order quote error");
                Metrics::get()
                    .update_result
                    .with_label_values(&["get_quote_preventable_failure"])
                    .inc();
                None
            }
            Err(err) => {
                tracing::debug!(?order_spec, ?err, "limit order unqoutable");
                Metrics::get()
                    .update_result
                    .with_label_values(&["get_quote_unpreventable_failure"])
                    .inc();
                None
            }
        }
    }

    /// Handles errors internally.
    async fn update_fee(&self, order_spec: &OrderFeeSpecifier, quote: &Option<Quote>) {
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
            .update_limit_order_fees(order_spec, &update)
            .await
        {
            Ok(_) => {
                Metrics::get()
                    .update_result
                    .with_label_values(&["update_fee_ok"])
                    .inc();
            }
            Err(err) => {
                tracing::warn!(?order_spec, ?err, "limit order fee update db error");
                Metrics::get()
                    .update_result
                    .with_label_values(&["update_fee_preventable_failure"])
                    .inc();
            }
        }
    }
}

fn query_from(data: &OrderQuotingData) -> Query {
    Query {
        owner: H160(data.owner.0),
        token: H160(data.sell_token.0),
        source: sell_token_source_from(data.sell_token_balance),
    }
}

fn order_spec_from(data: OrderQuotingData) -> OrderFeeSpecifier {
    OrderFeeSpecifier {
        sell_token: data.sell_token,
        buy_token: data.buy_token,
        sell_amount: data.sell_amount,
    }
}

async fn orders_with_sufficient_balance(
    balance_fetcher: &dyn BalanceFetching,
    mut orders: Vec<OrderQuotingData>,
) -> Vec<OrderQuotingData> {
    let queries = orders
        .iter()
        .filter(|order| order.pre_interactions <= 0)
        .map(query_from)
        .unique()
        .collect_vec();
    let balances = balance_fetcher.get_balances(&queries).await;
    let balances: HashMap<_, _> = queries
        .iter()
        .zip(balances.into_iter())
        .filter_map(|(query, result)| Some((query, result.ok()?)))
        .collect();

    let order_count_before = orders.len();

    orders.retain(|order| {
        if let Some(balance) = balances.get(&query_from(order)) {
            let has_sufficient_balance = match order.partially_fillable {
                true => balance >= &U256::one(), // any amount would be enough
                false => balance >= &big_decimal_to_u256(&order.sell_amount).unwrap(),
            };

            // It's possible that a pre_interaction transfers the owner the required balance
            // so we want to keep them even if the balance is insufficient at the moment.
            let could_transfer_money_in = order.pre_interactions > 0;
            return could_transfer_money_in || has_sufficient_balance;
        }

        // In case the balance couldn't be fetched err on the safe side and assume
        // the order can be filled to not discard limit orders unjustly.
        true
    });

    Metrics::get()
        .orders_skipped_for_missing_balance
        .set((order_count_before - orders.len()) as i64);
    orders
}

#[derive(Debug, Clone, Copy, Eq, Hash, PartialEq, clap::ValueEnum)]
#[clap(rename_all = "verbatim")]
pub enum QuotingStrategy {
    SkipUnfunded,
    // TODO add `PrioritizeByPrice`
}

#[derive(prometheus_metric_storage::MetricStorage, Clone, Debug)]
#[metric(subsystem = "limit_order_quoter")]
struct Metrics {
    /// Categorizes order quote update results.
    #[metric(labels("type"))]
    update_result: prometheus::IntCounterVec,

    /// Tracks how many orders don't get quoted because their
    /// owners don't have sufficient balance.
    orders_skipped_for_missing_balance: prometheus::IntGauge,
}

impl Metrics {
    fn get() -> &'static Self {
        Self::instance(global_metrics::get_metric_storage_registry()).unwrap()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use database::byte_array::ByteArray;
    use number_conversions::u256_to_big_decimal;
    use shared::account_balances::{MockBalanceFetching, Query};

    fn query(token: u8) -> Query {
        Query {
            owner: H160([1; 20]),
            token: H160([token; 20]),
            source: model::order::SellTokenSource::Erc20,
        }
    }

    #[tokio::test]
    async fn removes_orders_with_insufficient_balance() {
        let order = |sell_token, sell_amount: U256, partially_fillable, pre_interactions| {
            OrderQuotingData {
                owner: ByteArray([1; 20]),
                sell_token: ByteArray([sell_token; 20]),
                sell_amount: u256_to_big_decimal(&sell_amount),
                sell_token_balance: database::orders::SellTokenSource::Erc20,
                partially_fillable,
                pre_interactions,
                ..Default::default()
            }
        };

        let mut balance_fetcher = MockBalanceFetching::new();
        balance_fetcher
            .expect_get_balances()
            .withf(|arg| {
                arg == [
                    // Only 1 query for token 1 because identical balance queries get deduplicated.
                    query(1),
                    query(2),
                    query(3),
                ]
            })
            .returning(|_| {
                vec![
                    Ok(1_000.into()),
                    Ok(1.into()),
                    Err(anyhow::anyhow!("some error")),
                ]
            });

        let orders = vec![
            // Balance is sufficient.
            order(1, 1_000.into(), false, 0),
            // Balance is 1 short.
            order(1, 1_001.into(), false, 0),
            // For partially fillable orders a single atom is enough.
            order(2, U256::MAX, true, 0),
            // We always keep orders with pre_interactions in case they
            // would transfer enough money to the owner before settling the order.
            order(2, U256::MAX, false, 1),
            // We always keep orders where our balance request fails.
            order(3, U256::MAX, false, 0),
        ];

        let filtered_orders = orders_with_sufficient_balance(&balance_fetcher, orders).await;

        assert_eq!(
            filtered_orders,
            vec![
                order(1, 1_000.into(), false, 0),
                order(2, U256::MAX, true, 0),
                order(2, U256::MAX, false, 1),
                order(3, U256::MAX, false, 0),
            ]
        );
    }
}
