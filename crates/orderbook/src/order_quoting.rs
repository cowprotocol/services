use crate::{
    fee::{FeeData, FeeParameters, MinFeeCalculating},
    order_validation::{OrderValidating, PreOrderData, ValidationError},
};
use anyhow::Result;
use chrono::{DateTime, Utc};
use ethcontract::{H160, U256};
use futures::{try_join, TryFutureExt as _};
use gas_estimation::GasPriceEstimating;
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
use shared::price_estimation::{
    self,
    native::{native_single_estimate, NativePriceEstimating},
    single_estimate, PriceEstimating, PriceEstimationError,
};
use std::sync::{
    atomic::{AtomicU64, Ordering},
    Arc,
};
use thiserror::Error;

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
#[derive(Clone, Debug)]
pub struct QuoteParameters {
    pub sell_token: H160,
    pub buy_token: H160,
    pub side: OrderQuoteSide,
    pub from: H160,
    pub app_data: AppId,
}

impl QuoteParameters {
    fn to_price_query(&self) -> price_estimation::Query {
        let (kind, in_amount) = match self.side {
            OrderQuoteSide::Sell {
                sell_amount: SellAmount::BeforeFee { value: sell_amount },
            }
            | OrderQuoteSide::Sell {
                sell_amount: SellAmount::AfterFee { value: sell_amount },
            } => (OrderKind::Sell, sell_amount),
            OrderQuoteSide::Buy {
                buy_amount_after_fee,
            } => (OrderKind::Buy, buy_amount_after_fee),
        };

        price_estimation::Query {
            sell_token: self.sell_token,
            buy_token: self.buy_token,
            in_amount,
            kind,
        }
    }
}

/// A calculated order quote.
#[derive(Debug, Clone, PartialEq)]
pub struct Quote {
    pub id: QuoteId,
    pub data: QuoteData,
}

/// Detailed data for a computed order quote.
#[derive(Debug, Clone, PartialEq)]
pub struct QuoteData {
    pub sell_token: H160,
    pub buy_token: H160,
    pub sell_amount: U256,
    pub buy_amount: U256,
    pub fee_amount: U256,
    pub fee_parameters: FeeParameters,
    pub kind: OrderKind,
    pub expiration: DateTime<Utc>,
}

/// Quote searching parameters.
pub enum Search {
    Id(QuoteId),
    Parameters(QuoteSearchParameters),
}

#[cfg_attr(test, mockall::automock)]
#[async_trait::async_trait]
pub trait OrderQuoting: Send + Sync {
    /// Computes a quote for the specified order paramters.
    async fn calculate_quote(
        &self,
        parameters: QuoteParameters,
    ) -> Result<Quote, CalculateQuoteError>;

    /// Finds an existing quote.
    async fn find_quote(&self, search: Search) -> Result<Quote, FindQuoteError>;
}

#[derive(Error, Debug)]
pub enum CalculateQuoteError {
    #[error("sell amount does not cover fee")]
    SellAmountDoesNotCoverFee(U256),

    #[error("failed to estimate price")]
    Price(#[from] PriceEstimationError),

    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

#[derive(Error, Debug)]
pub enum FindQuoteError {
    #[error("quote not found")]
    NotFound(Option<QuoteId>),

    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

/// Fields for searching stored quotes.
pub struct QuoteSearchParameters {
    pub sell_token: H160,
    pub buy_token: H160,
    pub sell_amount: U256,
    pub buy_amount: U256,
    pub kind: OrderKind,
}

#[cfg_attr(test, mockall::automock)]
#[async_trait::async_trait]
pub trait QuoteStoring: Send + Sync {
    /// Saves a quote and returns its ID.
    async fn save(&self, data: QuoteData) -> Result<QuoteId>;

    /// Retrieves an existing quote by ID.
    async fn get(&self, id: QuoteId, expiration: DateTime<Utc>) -> Result<Option<QuoteData>>;

    /// Retrieves an existing quote by ID.
    async fn find(
        &self,
        parameters: QuoteSearchParameters,
        expiration: DateTime<Utc>,
    ) -> Result<Option<(QuoteId, QuoteData)>>;
}

#[cfg_attr(test, mockall::automock)]
pub trait Now: Send + Sync {
    fn now(&self) -> DateTime<Utc>;
}

impl<F> Now for F
where
    F: Fn() -> DateTime<Utc> + Send + Sync,
{
    fn now(&self) -> DateTime<Utc> {
        (self)()
    }
}

/// How long a quote remains valid for.
const QUOTE_VALIDITY_SECONDS: i64 = 60;

/// An order quoter implementation that relies
pub struct OrderQuoter2 {
    price_estimator: Arc<dyn PriceEstimating>,
    native_price_estimator: Arc<dyn NativePriceEstimating>,
    gas_estimator: Arc<dyn GasPriceEstimating>,
    storage: Arc<dyn QuoteStoring>,
    now: Arc<dyn Now>,
}

impl OrderQuoter2 {
    async fn compute_quote_data(
        &self,
        parameters: &QuoteParameters,
    ) -> Result<QuoteData, CalculateQuoteError> {
        let expiration = self.now.now() + chrono::Duration::seconds(QUOTE_VALIDITY_SECONDS);

        let trade_query = parameters.to_price_query();
        let (gas_estimate, trade_estimate, sell_token_price) = futures::try_join!(
            self.gas_estimator
                .estimate()
                .map_err(PriceEstimationError::from),
            single_estimate(self.price_estimator.as_ref(), &trade_query),
            native_single_estimate(self.native_price_estimator.as_ref(), &parameters.sell_token),
        )?;

        let fee_parameters = FeeParameters {
            gas_amount: trade_estimate.gas as _,
            gas_price: gas_estimate.effective_gas_price(),
            sell_token_price,
        };
        // TODO(nlordell): Apply subsidies!
        let fee_amount = fee_parameters.amount_in_sell_token();

        let (sell_amount, buy_amount) = match &parameters.side {
            OrderQuoteSide::Sell {
                sell_amount:
                    SellAmount::BeforeFee {
                        value: sell_amount_before_fee,
                    },
            } => {
                let sell_amount = sell_amount_before_fee.saturating_sub(fee_amount);
                if sell_amount == U256::zero() {
                    // We want a sell_amount of at least 1!
                    return Err(CalculateQuoteError::SellAmountDoesNotCoverFee(fee_amount));
                }

                let buy_amount = match trade_estimate.out_amount.checked_mul(sell_amount) {
                    Some(product) => product / sell_amount_before_fee,
                    // If we overflow when computing the product, use a different
                    // less precise method that avoids overflows.
                    None => (trade_estimate.out_amount / sell_amount_before_fee)
                        .checked_mul(sell_amount)
                        .unwrap_or(U256::MAX),
                };

                (sell_amount, buy_amount)
            }
            OrderQuoteSide::Sell {
                sell_amount: SellAmount::AfterFee { value: sell_amount },
            } => (*sell_amount, trade_estimate.out_amount),
            OrderQuoteSide::Buy {
                buy_amount_after_fee: buy_amount,
            } => (trade_estimate.out_amount, *buy_amount),
        };

        let quote = QuoteData {
            sell_token: parameters.sell_token,
            buy_token: parameters.buy_token,
            sell_amount,
            buy_amount,
            fee_amount,
            fee_parameters,
            kind: trade_query.kind,
            expiration,
        };

        tracing::debug!(?quote, "computed quote");
        Ok(quote)
    }
}

#[async_trait::async_trait]
impl OrderQuoting for OrderQuoter2 {
    async fn calculate_quote(
        &self,
        parameters: QuoteParameters,
    ) -> Result<Quote, CalculateQuoteError> {
        let data = self.compute_quote_data(&parameters).await?;
        let id = self.storage.save(data.clone()).await?;
        Ok(Quote { id, data })
    }

    async fn find_quote(&self, search: Search) -> Result<Quote, FindQuoteError> {
        let now = self.now.now();
        let quote = match search {
            Search::Id(id) => {
                let data = self
                    .storage
                    .get(id, now)
                    .await?
                    .ok_or(FindQuoteError::NotFound(Some(id)))?;
                Quote { id, data }
            }
            Search::Parameters(parameters) => {
                let (id, data) = self
                    .storage
                    .find(parameters, now)
                    .await?
                    .ok_or(FindQuoteError::NotFound(None))?;
                Quote { id, data }
            }
        };
        Ok(quote)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{fee::MockMinFeeCalculating, order_validation::MockOrderValidating};
    use chrono::Utc;
    use ethcontract::H160;
    use futures::{FutureExt as _, StreamExt as _};
    use gas_estimation::GasPrice1559;
    use model::{quote::Validity, time};
    use shared::{
        gas_price_estimation::FakeGasPriceEstimator,
        price_estimation::{
            mocks::FakePriceEstimator, native::MockNativePriceEstimating, MockPriceEstimating,
        },
    };
    use std::sync::Mutex;

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

    #[tokio::test]
    async fn compute_sell_before_fee_quote() {
        let start = Utc::now();
        let parameters = QuoteParameters {
            sell_token: H160([1; 20]),
            buy_token: H160([2; 20]),
            side: OrderQuoteSide::Sell {
                sell_amount: SellAmount::BeforeFee { value: 100.into() },
            },
            from: H160([3; 20]),
            app_data: AppId([4; 32]),
        };
        let gas_price = GasPrice1559 {
            base_fee_per_gas: 1.5,
            max_fee_per_gas: 3.0,
            max_priority_fee_per_gas: 0.5,
        };

        let mut price_estimator = MockPriceEstimating::new();
        price_estimator
            .expect_estimates()
            .withf(|q| {
                q == [price_estimation::Query {
                    sell_token: H160([1; 20]),
                    buy_token: H160([2; 20]),
                    in_amount: 100.into(),
                    kind: OrderKind::Sell,
                }]
            })
            .returning(|_| {
                futures::stream::iter([Ok(price_estimation::Estimate {
                    out_amount: 42.into(),
                    gas: 3,
                })])
                .enumerate()
                .boxed()
            });

        let mut native_price_estimator = MockNativePriceEstimating::new();
        native_price_estimator
            .expect_estimate_native_prices()
            .withf({
                let sell_token = parameters.sell_token;
                move |q| q == [sell_token]
            })
            .returning(|_| futures::stream::iter([Ok(0.2)]).enumerate().boxed());

        let gas_estimator = FakeGasPriceEstimator(Arc::new(Mutex::new(gas_price)));
        let mut now = MockNow::new();
        now.expect_now().returning(move || start);

        let quoter = OrderQuoter2 {
            price_estimator: Arc::new(price_estimator),
            native_price_estimator: Arc::new(native_price_estimator),
            gas_estimator: Arc::new(gas_estimator),
            storage: Arc::new(MockQuoteStoring::new()),
            now: Arc::new(now),
        };

        assert_eq!(
            quoter.compute_quote_data(&parameters).await.unwrap(),
            QuoteData {
                sell_token: H160([1; 20]),
                buy_token: H160([2; 20]),
                sell_amount: 70.into(),
                buy_amount: 29.into(),
                fee_amount: 30.into(),
                fee_parameters: FeeParameters {
                    gas_amount: 3.,
                    gas_price: 2.,
                    sell_token_price: 0.2,
                },
                kind: OrderKind::Sell,
                expiration: start + chrono::Duration::seconds(QUOTE_VALIDITY_SECONDS),
            }
        );
    }

    #[tokio::test]
    async fn compute_sell_after_fee_quote() {
        let start = Utc::now();
        let parameters = QuoteParameters {
            sell_token: H160([1; 20]),
            buy_token: H160([2; 20]),
            side: OrderQuoteSide::Sell {
                sell_amount: SellAmount::AfterFee { value: 100.into() },
            },
            from: H160([3; 20]),
            app_data: AppId([4; 32]),
        };
        let gas_price = GasPrice1559 {
            base_fee_per_gas: 1.5,
            max_fee_per_gas: 3.0,
            max_priority_fee_per_gas: 0.5,
        };

        let mut price_estimator = MockPriceEstimating::new();
        price_estimator
            .expect_estimates()
            .withf(|q| {
                q == [price_estimation::Query {
                    sell_token: H160([1; 20]),
                    buy_token: H160([2; 20]),
                    in_amount: 100.into(),
                    kind: OrderKind::Sell,
                }]
            })
            .returning(|_| {
                futures::stream::iter([Ok(price_estimation::Estimate {
                    out_amount: 42.into(),
                    gas: 3,
                })])
                .enumerate()
                .boxed()
            });

        let mut native_price_estimator = MockNativePriceEstimating::new();
        native_price_estimator
            .expect_estimate_native_prices()
            .withf({
                let sell_token = parameters.sell_token;
                move |q| q == [sell_token]
            })
            .returning(|_| futures::stream::iter([Ok(0.2)]).enumerate().boxed());

        let gas_estimator = FakeGasPriceEstimator(Arc::new(Mutex::new(gas_price)));
        let mut now = MockNow::new();
        now.expect_now().returning(move || start);

        let quoter = OrderQuoter2 {
            price_estimator: Arc::new(price_estimator),
            native_price_estimator: Arc::new(native_price_estimator),
            gas_estimator: Arc::new(gas_estimator),
            storage: Arc::new(MockQuoteStoring::new()),
            now: Arc::new(now),
        };

        assert_eq!(
            quoter.compute_quote_data(&parameters).await.unwrap(),
            QuoteData {
                sell_token: H160([1; 20]),
                buy_token: H160([2; 20]),
                sell_amount: 100.into(),
                buy_amount: 42.into(),
                fee_amount: 30.into(),
                fee_parameters: FeeParameters {
                    gas_amount: 3.,
                    gas_price: 2.,
                    sell_token_price: 0.2,
                },
                kind: OrderKind::Sell,
                expiration: start + chrono::Duration::seconds(QUOTE_VALIDITY_SECONDS),
            }
        );
    }

    #[tokio::test]
    async fn compute_buy_quote() {
        let start = Utc::now();
        let parameters = QuoteParameters {
            sell_token: H160([1; 20]),
            buy_token: H160([2; 20]),
            side: OrderQuoteSide::Buy {
                buy_amount_after_fee: 42.into(),
            },
            from: H160([3; 20]),
            app_data: AppId([4; 32]),
        };
        let gas_price = GasPrice1559 {
            base_fee_per_gas: 1.5,
            max_fee_per_gas: 3.0,
            max_priority_fee_per_gas: 0.5,
        };

        let mut price_estimator = MockPriceEstimating::new();
        price_estimator
            .expect_estimates()
            .withf(|q| {
                q == [price_estimation::Query {
                    sell_token: H160([1; 20]),
                    buy_token: H160([2; 20]),
                    in_amount: 42.into(),
                    kind: OrderKind::Buy,
                }]
            })
            .returning(|_| {
                futures::stream::iter([Ok(price_estimation::Estimate {
                    out_amount: 100.into(),
                    gas: 3,
                })])
                .enumerate()
                .boxed()
            });

        let mut native_price_estimator = MockNativePriceEstimating::new();
        native_price_estimator
            .expect_estimate_native_prices()
            .withf({
                let sell_token = parameters.sell_token;
                move |q| q == [sell_token]
            })
            .returning(|_| futures::stream::iter([Ok(0.2)]).enumerate().boxed());

        let gas_estimator = FakeGasPriceEstimator(Arc::new(Mutex::new(gas_price)));
        let mut now = MockNow::new();
        now.expect_now().returning(move || start);

        let quoter = OrderQuoter2 {
            price_estimator: Arc::new(price_estimator),
            native_price_estimator: Arc::new(native_price_estimator),
            gas_estimator: Arc::new(gas_estimator),
            storage: Arc::new(MockQuoteStoring::new()),
            now: Arc::new(now),
        };

        assert_eq!(
            quoter.compute_quote_data(&parameters).await.unwrap(),
            QuoteData {
                sell_token: H160([1; 20]),
                buy_token: H160([2; 20]),
                sell_amount: 100.into(),
                buy_amount: 42.into(),
                fee_amount: 30.into(),
                fee_parameters: FeeParameters {
                    gas_amount: 3.,
                    gas_price: 2.,
                    sell_token_price: 0.2,
                },
                kind: OrderKind::Buy,
                expiration: start + chrono::Duration::seconds(QUOTE_VALIDITY_SECONDS),
            }
        );
    }

    #[tokio::test]
    async fn compute_sell_before_fee_quote_insufficient_amount_error() {
        let parameters = QuoteParameters {
            sell_token: H160([1; 20]),
            buy_token: H160([2; 20]),
            side: OrderQuoteSide::Sell {
                sell_amount: SellAmount::BeforeFee { value: 100.into() },
            },
            from: H160([3; 20]),
            app_data: AppId([4; 32]),
        };
        let gas_price = GasPrice1559 {
            base_fee_per_gas: 1.,
            max_fee_per_gas: 2.,
            max_priority_fee_per_gas: 0.,
        };

        let mut price_estimator = MockPriceEstimating::new();
        price_estimator.expect_estimates().returning(|_| {
            futures::stream::iter([Ok(price_estimation::Estimate {
                out_amount: 100.into(),
                gas: 200,
            })])
            .enumerate()
            .boxed()
        });

        let mut native_price_estimator = MockNativePriceEstimating::new();
        native_price_estimator
            .expect_estimate_native_prices()
            .withf({
                let sell_token = parameters.sell_token;
                move |q| q == [sell_token]
            })
            .returning(|_| futures::stream::iter([Ok(1.)]).enumerate().boxed());

        let gas_estimator = FakeGasPriceEstimator(Arc::new(Mutex::new(gas_price)));

        let quoter = OrderQuoter2 {
            price_estimator: Arc::new(price_estimator),
            native_price_estimator: Arc::new(native_price_estimator),
            gas_estimator: Arc::new(gas_estimator),
            storage: Arc::new(MockQuoteStoring::new()),
            now: Arc::new(Utc::now),
        };

        assert!(matches!(
            quoter.compute_quote_data(&parameters).await.unwrap_err(),
            CalculateQuoteError::SellAmountDoesNotCoverFee(fee) if fee == U256::from(200),
        ));
    }
}
