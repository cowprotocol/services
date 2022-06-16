use crate::{
    fee::{FeeData, FeeParameters, MinFeeCalculating},
    order_validation::{OrderValidating, PreOrderData, ValidationError},
};
use anyhow::Result;
use chrono::{DateTime, Utc};
use ethcontract::{H160, U256};
use futures::try_join;
use model::{
    app_id::AppId,
    order::OrderKind,
    quote::{
        OrderQuote, OrderQuoteRequest, OrderQuoteResponse, OrderQuoteSide, PriceQuality, QuoteId,
        SellAmount,
    },
    u256_decimal,
};
use serde::Serialize;
use shared::price_estimation::{self, single_estimate, PriceEstimating, PriceEstimationError};
use std::sync::{
    atomic::{AtomicU64, Ordering},
    Arc,
};

impl From<&OrderQuoteRequest> for PreOrderData {
    fn from(quote_request: &OrderQuoteRequest) -> Self {
        let owner = quote_request.from;
        Self {
            owner,
            sell_token: quote_request.sell_token,
            buy_token: quote_request.buy_token,
            receiver: quote_request.receiver.unwrap_or(owner),
            valid_to: quote_request.validity.actual_valid_to(),
            partially_fillable: quote_request.partially_fillable,
            buy_token_balance: quote_request.buy_token_balance,
            sell_token_balance: quote_request.sell_token_balance,
            signing_scheme: quote_request.signing_scheme,
            is_liquidity_order: quote_request.partially_fillable,
        }
    }
}

#[derive(Debug)]
pub enum FeeError {
    SellAmountDoesNotCoverFee(FeeInfo),
    PriceEstimate(PriceEstimationError),
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FeeInfo {
    #[serde(with = "u256_decimal")]
    pub fee_amount: U256,
    pub expiration: DateTime<Utc>,
}

#[derive(Debug)]
pub enum OrderQuoteError {
    Fee(FeeError),
    Order(ValidationError),
}

#[derive(Debug, Serialize, PartialEq)]
struct QuoteFeeParameters {
    buy_amount: U256,
    sell_amount: U256,
    fee_amount: U256,
    expiration: DateTime<Utc>,
    kind: OrderKind,
}

#[derive(Clone)]
pub struct OrderQuoter {
    pub fee_calculator: Arc<dyn MinFeeCalculating>,
    pub price_estimator: Arc<dyn PriceEstimating>,
    pub order_validator: Arc<dyn OrderValidating>,
    pub fast_fee_calculator: Arc<dyn MinFeeCalculating>,
    pub fast_price_estimator: Arc<dyn PriceEstimating>,
    pub current_id: Arc<AtomicU64>,
}

impl OrderQuoter {
    pub fn new(
        fee_calculator: Arc<dyn MinFeeCalculating>,
        price_estimator: Arc<dyn PriceEstimating>,
        order_validator: Arc<dyn OrderValidating>,
    ) -> Self {
        Self {
            fast_fee_calculator: fee_calculator.clone(),
            fast_price_estimator: price_estimator.clone(),
            fee_calculator,
            price_estimator,
            order_validator,
            current_id: Default::default(),
        }
    }

    pub fn with_fast_quotes(
        mut self,
        fee_calculator: Arc<dyn MinFeeCalculating>,
        price_estimator: Arc<dyn PriceEstimating>,
    ) -> Self {
        self.fast_fee_calculator = fee_calculator;
        self.fast_price_estimator = price_estimator;
        self
    }

    pub async fn calculate_quote(
        &self,
        quote_request: &OrderQuoteRequest,
    ) -> Result<OrderQuoteResponse, OrderQuoteError> {
        tracing::debug!("Received quote request {:?}", quote_request);
        let pre_order = PreOrderData::from(quote_request);
        let valid_to = pre_order.valid_to;
        self.order_validator
            .partial_validate(pre_order)
            .await
            .map_err(|err| OrderQuoteError::Order(ValidationError::Partial(err)))?;
        let fee_parameters = self
            .calculate_fee_parameters(quote_request)
            .await
            .map_err(OrderQuoteError::Fee)?;
        Ok(OrderQuoteResponse {
            quote: OrderQuote {
                sell_token: quote_request.sell_token,
                buy_token: quote_request.buy_token,
                receiver: quote_request.receiver,
                sell_amount: fee_parameters.sell_amount,
                buy_amount: fee_parameters.buy_amount,
                valid_to,
                app_data: quote_request.app_data,
                fee_amount: fee_parameters.fee_amount,
                kind: fee_parameters.kind,
                partially_fillable: quote_request.partially_fillable,
                sell_token_balance: quote_request.sell_token_balance,
                buy_token_balance: quote_request.buy_token_balance,
            },
            from: quote_request.from,
            expiration: fee_parameters.expiration,
            id: self.current_id.fetch_add(1, Ordering::SeqCst),
        })
    }

    async fn calculate_fee_parameters(
        &self,
        quote_request: &OrderQuoteRequest,
    ) -> Result<QuoteFeeParameters, FeeError> {
        let (fee_calculator, price_estimator) = match quote_request.price_quality {
            PriceQuality::Fast => (&self.fast_fee_calculator, &self.fast_price_estimator),
            PriceQuality::Optimal => (&self.fee_calculator, &self.price_estimator),
        };

        Ok(match quote_request.side {
            OrderQuoteSide::Sell {
                sell_amount:
                    SellAmount::BeforeFee {
                        value: sell_amount_before_fee,
                    },
            } => {
                if sell_amount_before_fee.is_zero() {
                    return Err(FeeError::PriceEstimate(PriceEstimationError::ZeroAmount));
                }
                let query = price_estimation::Query {
                    // It would be more correct to use sell_amount_after_fee here, however this makes the two long-running fee and price estimation queries dependent and causes very long roundtrip times
                    // We therefore compute the exchange rate for the sell_amount_before_fee and assume that the price for selling a smaller amount (after fee) will be close to but at least as good
                    sell_token: quote_request.sell_token,
                    buy_token: quote_request.buy_token,
                    in_amount: sell_amount_before_fee,
                    kind: OrderKind::Sell,
                };
                let ((fee, expiration), estimate) = try_join!(
                    fee_calculator.compute_subsidized_min_fee(
                        FeeData {
                            sell_token: quote_request.sell_token,
                            buy_token: quote_request.buy_token,
                            amount: sell_amount_before_fee,
                            kind: OrderKind::Sell,
                        },
                        quote_request.app_data,
                        quote_request.from,
                    ),
                    single_estimate(price_estimator.as_ref(), &query)
                )
                .map_err(FeeError::PriceEstimate)?;
                let sell_amount_after_fee = sell_amount_before_fee
                    .checked_sub(fee)
                    .ok_or(FeeError::SellAmountDoesNotCoverFee(FeeInfo {
                        fee_amount: fee,
                        expiration,
                    }))?
                    .max(U256::one());
                let buy_amount_after_fee =
                    match estimate.out_amount.checked_mul(sell_amount_after_fee) {
                        // sell_amount_before_fee is at least 1 (cf. above)
                        Some(product) => product / sell_amount_before_fee,
                        None => (estimate.out_amount / sell_amount_before_fee)
                            .checked_mul(sell_amount_after_fee)
                            .unwrap_or(U256::MAX),
                    };
                QuoteFeeParameters {
                    buy_amount: buy_amount_after_fee,
                    sell_amount: sell_amount_after_fee,
                    fee_amount: fee,
                    expiration,
                    kind: OrderKind::Sell,
                }
            }
            OrderQuoteSide::Sell {
                sell_amount:
                    SellAmount::AfterFee {
                        value: sell_amount_after_fee,
                    },
            } => {
                if sell_amount_after_fee.is_zero() {
                    return Err(FeeError::PriceEstimate(PriceEstimationError::ZeroAmount));
                }

                let price_estimation_query = price_estimation::Query {
                    sell_token: quote_request.sell_token,
                    buy_token: quote_request.buy_token,
                    in_amount: sell_amount_after_fee,
                    kind: OrderKind::Sell,
                };

                // Since both futures are long running and independent, run concurrently
                let ((fee, expiration), estimate) = try_join!(
                    fee_calculator.compute_subsidized_min_fee(
                        FeeData {
                            sell_token: quote_request.sell_token,
                            buy_token: quote_request.buy_token,
                            amount: sell_amount_after_fee,
                            kind: OrderKind::Sell,
                        },
                        quote_request.app_data,
                        quote_request.from,
                    ),
                    single_estimate(price_estimator.as_ref(), &price_estimation_query)
                )
                .map_err(FeeError::PriceEstimate)?;
                QuoteFeeParameters {
                    buy_amount: estimate.out_amount,
                    sell_amount: sell_amount_after_fee,
                    fee_amount: fee,
                    expiration,
                    kind: OrderKind::Sell,
                }
            }
            OrderQuoteSide::Buy {
                buy_amount_after_fee,
            } => {
                if buy_amount_after_fee.is_zero() {
                    return Err(FeeError::PriceEstimate(PriceEstimationError::ZeroAmount));
                }

                let price_estimation_query = price_estimation::Query {
                    sell_token: quote_request.sell_token,
                    buy_token: quote_request.buy_token,
                    in_amount: buy_amount_after_fee,
                    kind: OrderKind::Buy,
                };

                // Since both futures are long running and independent, run concurrently
                let ((fee, expiration), estimate) = try_join!(
                    fee_calculator.compute_subsidized_min_fee(
                        FeeData {
                            sell_token: quote_request.sell_token,
                            buy_token: quote_request.buy_token,
                            amount: buy_amount_after_fee,
                            kind: OrderKind::Buy,
                        },
                        quote_request.app_data,
                        quote_request.from,
                    ),
                    single_estimate(price_estimator.as_ref(), &price_estimation_query)
                )
                .map_err(FeeError::PriceEstimate)?;
                let sell_amount_after_fee = estimate.out_amount;
                QuoteFeeParameters {
                    buy_amount: buy_amount_after_fee,
                    sell_amount: sell_amount_after_fee,
                    fee_amount: fee,
                    expiration,
                    kind: OrderKind::Buy,
                }
            }
        })
    }
}

/// Order parameters for quoting.
pub struct QuoteParameters {
    pub sell_token: H160,
    pub buy_token: H160,
    pub side: OrderQuoteSide,
    pub from: H160,
    pub app_data: AppId,
}

/// Detailed information for a computed order quote.
#[derive(Debug, Clone, PartialEq)]
pub struct Quote {
    pub id: QuoteId,
    pub expiry: DateTime<Utc>,
    pub sell_token: H160,
    pub buy_token: H160,
    pub sell_amount: U256,
    pub buy_amount: U256,
    pub fee_amount: U256,
    pub fee_parameters: FeeParameters,
    pub kind: OrderKind,
}

/// Quote searching parameters.
pub enum Search {
    Id(QuoteId),
    Parameters(QuoteParameters),
}

#[cfg_attr(test, mockall::automock)]
#[async_trait::async_trait]
pub trait OrderQuoteCalculating: Send + Sync {
    /// Computes a quote for the specified order paramters.
    ///
    /// Returns an error if there is some estimation error and `Ok(None)` if no
    /// information about the given token exists
    async fn calculate_quote(
        &self,
        parameters: QuoteParameters,
    ) -> Result<Quote, CalculateQuoteError>;

    /// Finds an existing quote.
    async fn find_quote(&self, search: Search) -> Result<Quote, FindQuoteError>;
}

pub enum CalculateQuoteError {}

pub enum FindQuoteError {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{fee::MockMinFeeCalculating, order_validation::MockOrderValidating};
    use chrono::Utc;
    use ethcontract::H160;
    use futures::FutureExt;
    use model::{quote::Validity, time};
    use shared::price_estimation::mocks::FakePriceEstimator;

    #[test]
    fn calculate_fee_sell_before_fees_quote_request() {
        let mut fee_calculator = MockMinFeeCalculating::new();

        let expiration = Utc::now();
        fee_calculator
            .expect_compute_subsidized_min_fee()
            .returning(move |_, _, _| Ok((3.into(), expiration)));

        let fee_calculator = Arc::new(fee_calculator);
        let price_estimator = FakePriceEstimator(price_estimation::Estimate {
            out_amount: 14.into(),
            gas: 1000,
        });
        let sell_query = OrderQuoteRequest::new(
            H160::from_low_u64_ne(0),
            H160::from_low_u64_ne(1),
            OrderQuoteSide::Sell {
                sell_amount: SellAmount::BeforeFee { value: 10.into() },
            },
        );
        let quoter = Arc::new(OrderQuoter::new(
            fee_calculator,
            Arc::new(price_estimator),
            Arc::new(MockOrderValidating::new()),
        ));
        let result = quoter
            .calculate_fee_parameters(&sell_query)
            .now_or_never()
            .unwrap()
            .unwrap();
        // After the deducting the fee 10 - 3 = 7 units of sell token are being sold.
        // Selling 10 units will buy us 14. Therefore, selling 7 should buy us 9.8 => 9 whole units
        assert_eq!(
            result,
            QuoteFeeParameters {
                buy_amount: 9.into(),
                sell_amount: 7.into(),
                fee_amount: 3.into(),
                expiration,
                kind: OrderKind::Sell
            }
        );
    }

    #[test]
    fn calculate_fee_sell_after_fees_quote_request() {
        let mut fee_calculator = MockMinFeeCalculating::new();
        let expiration = Utc::now();
        fee_calculator
            .expect_compute_subsidized_min_fee()
            .returning(move |_, _, _| Ok((3.into(), expiration)));

        let fee_calculator = Arc::new(fee_calculator);
        let price_estimator = FakePriceEstimator(price_estimation::Estimate {
            out_amount: 14.into(),
            gas: 1000,
        });
        let sell_query = OrderQuoteRequest::new(
            H160::from_low_u64_ne(0),
            H160::from_low_u64_ne(1),
            OrderQuoteSide::Sell {
                sell_amount: SellAmount::AfterFee { value: 7.into() },
            },
        );

        let quoter = Arc::new(OrderQuoter::new(
            fee_calculator,
            Arc::new(price_estimator),
            Arc::new(MockOrderValidating::new()),
        ));
        let result = quoter
            .calculate_fee_parameters(&sell_query)
            .now_or_never()
            .unwrap()
            .unwrap();
        assert_eq!(
            result,
            QuoteFeeParameters {
                buy_amount: 14.into(),
                sell_amount: 7.into(),
                fee_amount: 3.into(),
                expiration,
                kind: OrderKind::Sell
            }
        );
    }

    #[test]
    fn calculate_fee_buy_quote_request() {
        let mut fee_calculator = MockMinFeeCalculating::new();
        let expiration = Utc::now();
        fee_calculator
            .expect_compute_subsidized_min_fee()
            .returning(move |_, _, _| Ok((3.into(), expiration)));

        let fee_calculator = Arc::new(fee_calculator);
        let price_estimator = FakePriceEstimator(price_estimation::Estimate {
            out_amount: 20.into(),
            gas: 1000,
        });
        let buy_query = OrderQuoteRequest::new(
            H160::from_low_u64_ne(0),
            H160::from_low_u64_ne(1),
            OrderQuoteSide::Buy {
                buy_amount_after_fee: 10.into(),
            },
        );
        let quoter = Arc::new(OrderQuoter::new(
            fee_calculator,
            Arc::new(price_estimator),
            Arc::new(MockOrderValidating::new()),
        ));
        let result = quoter
            .calculate_fee_parameters(&buy_query)
            .now_or_never()
            .unwrap()
            .unwrap();
        // To buy 10 units of buy_token the fee in sell_token must be at least 3 and at least 20
        // units of sell_token must be sold.
        assert_eq!(
            result,
            QuoteFeeParameters {
                buy_amount: 10.into(),
                sell_amount: 20.into(),
                fee_amount: 3.into(),
                expiration,
                kind: OrderKind::Buy
            }
        );
    }

    #[test]
    fn pre_order_data_from_quote_request() {
        let quote_request = OrderQuoteRequest {
            validity: Validity::To(0),
            ..Default::default()
        };
        let result = PreOrderData::from(&quote_request);
        let expected = PreOrderData::default();
        assert_eq!(result, expected);
    }

    #[test]
    fn pre_order_data_from_quote_request_with_valid_for() {
        let quote_request = OrderQuoteRequest {
            validity: Validity::For(100),
            ..Default::default()
        };
        let result = PreOrderData::from(&quote_request);

        let valid_duration = result.valid_to - time::now_in_epoch_seconds();

        // use time-range to make sure test isn't flaky.
        assert!((95..=105).contains(&valid_duration));
    }

    #[tokio::test]
    async fn calculate_quote() {
        let buy_request = OrderQuoteRequest {
            sell_token: H160::from_low_u64_be(1),
            buy_token: H160::from_low_u64_be(2),
            side: OrderQuoteSide::Buy {
                buy_amount_after_fee: 2.into(),
            },
            validity: Validity::To(0),
            ..Default::default()
        };

        let mut fee_calculator = MockMinFeeCalculating::new();
        fee_calculator
            .expect_compute_subsidized_min_fee()
            .returning(move |_, _, _| Ok((3.into(), Utc::now())));
        let price_estimator = FakePriceEstimator(price_estimation::Estimate {
            out_amount: 14.into(),
            gas: 1000,
        });
        let mut order_validator = MockOrderValidating::new();
        order_validator
            .expect_partial_validate()
            .returning(|_| Ok(()));
        let quoter = Arc::new(OrderQuoter::new(
            Arc::new(fee_calculator),
            Arc::new(price_estimator),
            Arc::new(order_validator),
        ));
        let result = quoter.calculate_quote(&buy_request).await.unwrap();

        let expected = OrderQuote {
            sell_token: H160::from_low_u64_be(1),
            buy_token: H160::from_low_u64_be(2),
            receiver: None,
            sell_amount: 14.into(),
            kind: OrderKind::Buy,
            partially_fillable: false,
            sell_token_balance: Default::default(),
            buy_amount: 2.into(),
            valid_to: 0,
            app_data: Default::default(),
            fee_amount: 3.into(),
            buy_token_balance: Default::default(),
        };
        assert_eq!(result.quote, expected);
    }
}
