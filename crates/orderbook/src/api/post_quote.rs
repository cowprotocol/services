use crate::{
    api::{
        self, convert_json_response,
        order_validation::{OrderValidating, PreOrderData, ValidationError},
        IntoWarpReply,
    },
    fee::{FeeData, MinFeeCalculating, PriceQuality},
};
use anyhow::Result;
use chrono::{DateTime, Utc};
use ethcontract::{H160, U256};
use futures::try_join;
use model::{
    app_id::AppId,
    order::{BuyTokenDestination, OrderKind, SellTokenSource},
    u256_decimal,
};
use serde::{Deserialize, Serialize};
use shared::price_estimation::{self, PriceEstimating, PriceEstimationError};
use std::{convert::Infallible, sync::Arc};
use warp::{hyper::StatusCode, Filter, Rejection};

/// The order parameters to quote a price and fee for.
#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct OrderQuoteRequest {
    from: H160,
    sell_token: H160,
    buy_token: H160,
    receiver: Option<H160>,
    #[serde(flatten)]
    side: OrderQuoteSide,
    valid_to: u32,
    app_data: AppId,
    partially_fillable: bool,
    #[serde(default)]
    sell_token_balance: SellTokenSource,
    #[serde(default)]
    buy_token_balance: BuyTokenDestination,
    #[serde(default)]
    price_quality: PriceQuality,
}

impl From<&OrderQuoteRequest> for PreOrderData {
    fn from(quote_request: &OrderQuoteRequest) -> Self {
        let owner = quote_request.from;
        Self {
            owner,
            sell_token: quote_request.sell_token,
            buy_token: quote_request.buy_token,
            receiver: quote_request.receiver.unwrap_or(owner),
            valid_to: quote_request.valid_to,
            partially_fillable: quote_request.partially_fillable,
            buy_token_balance: quote_request.buy_token_balance,
            sell_token_balance: quote_request.sell_token_balance,
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum OrderQuoteSide {
    #[serde(rename_all = "camelCase")]
    Sell {
        #[serde(flatten)]
        sell_amount: SellAmount,
    },
    #[serde(rename_all = "camelCase")]
    Buy {
        #[serde(with = "u256_decimal")]
        buy_amount_after_fee: U256,
    },
}

impl Default for OrderQuoteSide {
    fn default() -> Self {
        Self::Buy {
            buy_amount_after_fee: U256::one(),
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq)]
#[serde(untagged)]
pub enum SellAmount {
    BeforeFee {
        #[serde(rename = "sellAmountBeforeFee", with = "u256_decimal")]
        value: U256,
    },
    AfterFee {
        #[serde(rename = "sellAmountAfterFee", with = "u256_decimal")]
        value: U256,
    },
}

/// The quoted order by the service.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OrderQuote {
    pub sell_token: H160,
    pub buy_token: H160,
    pub receiver: Option<H160>,
    #[serde(with = "u256_decimal")]
    pub sell_amount: U256,
    #[serde(with = "u256_decimal")]
    pub buy_amount: U256,
    pub valid_to: u32,
    pub app_data: AppId,
    #[serde(with = "u256_decimal")]
    pub fee_amount: U256,
    pub kind: OrderKind,
    pub partially_fillable: bool,
    pub sell_token_balance: SellTokenSource,
    pub buy_token_balance: BuyTokenDestination,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OrderQuoteResponse {
    pub quote: OrderQuote,
    pub from: H160,
    pub expiration: DateTime<Utc>,
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

impl IntoWarpReply for FeeError {
    fn into_warp_reply(self) -> super::ApiReply {
        match self {
            FeeError::PriceEstimate(err) => err.into_warp_reply(),
            FeeError::SellAmountDoesNotCoverFee(fee) => warp::reply::with_status(
                super::rich_error(
                    "SellAmountDoesNotCoverFee",
                    "The sell amount for the sell order is lower than the fee.".to_string(),
                    fee,
                ),
                StatusCode::BAD_REQUEST,
            ),
        }
    }
}

#[derive(Debug)]
pub enum OrderQuoteError {
    Fee(FeeError),
    Order(ValidationError),
}

impl IntoWarpReply for OrderQuoteError {
    fn into_warp_reply(self) -> super::ApiReply {
        match self {
            OrderQuoteError::Fee(err) => err.into_warp_reply(),
            OrderQuoteError::Order(err) => err.into_warp_reply(),
        }
    }
}

#[derive(Debug, Serialize, PartialEq)]
struct FeeParameters {
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
        self.order_validator
            .partial_validate(quote_request.into())
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
                valid_to: quote_request.valid_to,
                app_data: quote_request.app_data,
                fee_amount: fee_parameters.fee_amount,
                kind: fee_parameters.kind,
                partially_fillable: quote_request.partially_fillable,
                sell_token_balance: quote_request.sell_token_balance,
                buy_token_balance: quote_request.buy_token_balance,
            },
            from: quote_request.from,
            expiration: fee_parameters.expiration,
        })
    }

    async fn calculate_fee_parameters(
        &self,
        quote_request: &OrderQuoteRequest,
    ) -> Result<FeeParameters, FeeError> {
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
                    ),
                    price_estimator.estimate(&query)
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
                FeeParameters {
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
                    ),
                    price_estimator.estimate(&price_estimation_query)
                )
                .map_err(FeeError::PriceEstimate)?;
                FeeParameters {
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
                    ),
                    price_estimator.estimate(&price_estimation_query)
                )
                .map_err(FeeError::PriceEstimate)?;
                let sell_amount_after_fee = estimate.out_amount;
                FeeParameters {
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

impl OrderQuoteRequest {
    /// This method is used by the old, deprecated, fee endpoint to convert {Buy, Sell}Requests
    pub fn new(sell_token: H160, buy_token: H160, side: OrderQuoteSide) -> Self {
        Self {
            sell_token,
            buy_token,
            side,
            valid_to: u32::MAX,
            ..Default::default()
        }
    }
}

fn post_quote_request() -> impl Filter<Extract = (OrderQuoteRequest,), Error = Rejection> + Clone {
    warp::path!("quote")
        .and(warp::post())
        .and(api::extract_payload())
}

pub fn post_quote(
    quoter: Arc<OrderQuoter>,
) -> impl Filter<Extract = (super::ApiReply,), Error = Rejection> + Clone {
    post_quote_request().and_then(move |request: OrderQuoteRequest| {
        let quoter = quoter.clone();
        async move {
            let result = quoter.calculate_quote(&request).await;
            if let Err(err) = &result {
                tracing::warn!(?err, ?request, "post_quote error");
            }
            Result::<_, Infallible>::Ok(convert_json_response(result))
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        api::{order_validation::MockOrderValidating, response_body},
        fee::MockMinFeeCalculating,
    };
    use anyhow::anyhow;
    use chrono::{NaiveDateTime, Utc};
    use futures::FutureExt;
    use serde_json::json;
    use shared::price_estimation::mocks::FakePriceEstimator;
    use warp::{test::request, Reply};

    #[test]
    fn deserializes_sell_after_fees_quote_request() {
        assert_eq!(
            serde_json::from_value::<OrderQuoteRequest>(json!({
                "from": "0x0101010101010101010101010101010101010101",
                "sellToken": "0x0202020202020202020202020202020202020202",
                "buyToken": "0x0303030303030303030303030303030303030303",
                "kind": "sell",
                "sellAmountAfterFee": "1337",
                "validTo": 0x12345678,
                "appData": "0x9090909090909090909090909090909090909090909090909090909090909090",
                "partiallyFillable": false,
                "buyTokenBalance": "internal",
                "priceQuality": "optimal"
            }))
            .unwrap(),
            OrderQuoteRequest {
                from: H160([0x01; 20]),
                sell_token: H160([0x02; 20]),
                buy_token: H160([0x03; 20]),
                receiver: None,
                side: OrderQuoteSide::Sell {
                    sell_amount: SellAmount::AfterFee { value: 1337.into() },
                },
                valid_to: 0x12345678,
                app_data: AppId([0x90; 32]),
                partially_fillable: false,
                sell_token_balance: SellTokenSource::Erc20,
                buy_token_balance: BuyTokenDestination::Internal,
                price_quality: PriceQuality::Optimal
            }
        );
    }

    #[test]
    fn deserializes_sell_before_fees_quote_request() {
        assert_eq!(
            serde_json::from_value::<OrderQuoteRequest>(json!({
                "from": "0x0101010101010101010101010101010101010101",
                "sellToken": "0x0202020202020202020202020202020202020202",
                "buyToken": "0x0303030303030303030303030303030303030303",
                "kind": "sell",
                "sellAmountBeforeFee": "1337",
                "validTo": 0x12345678,
                "appData": "0x9090909090909090909090909090909090909090909090909090909090909090",
                "partiallyFillable": false,
                "sellTokenBalance": "external",
                "priceQuality": "fast"
            }))
            .unwrap(),
            OrderQuoteRequest {
                from: H160([0x01; 20]),
                sell_token: H160([0x02; 20]),
                buy_token: H160([0x03; 20]),
                receiver: None,
                side: OrderQuoteSide::Sell {
                    sell_amount: SellAmount::BeforeFee { value: 1337.into() },
                },
                valid_to: 0x12345678,
                app_data: AppId([0x90; 32]),
                partially_fillable: false,
                sell_token_balance: SellTokenSource::External,
                buy_token_balance: BuyTokenDestination::Erc20,
                price_quality: PriceQuality::Fast
            }
        );
    }

    #[test]
    fn deserializes_buy_quote_request() {
        assert_eq!(
            serde_json::from_value::<OrderQuoteRequest>(json!({
                "from": "0x0101010101010101010101010101010101010101",
                "sellToken": "0x0202020202020202020202020202020202020202",
                "buyToken": "0x0303030303030303030303030303030303030303",
                "receiver": "0x0404040404040404040404040404040404040404",
                "kind": "buy",
                "buyAmountAfterFee": "1337",
                "validTo": 0x12345678,
                "appData": "0x9090909090909090909090909090909090909090909090909090909090909090",
                "partiallyFillable": false,
            }))
            .unwrap(),
            OrderQuoteRequest {
                from: H160([0x01; 20]),
                sell_token: H160([0x02; 20]),
                buy_token: H160([0x03; 20]),
                receiver: Some(H160([0x04; 20])),
                side: OrderQuoteSide::Buy {
                    buy_amount_after_fee: U256::from(1337),
                },
                valid_to: 0x12345678,
                app_data: AppId([0x90; 32]),
                partially_fillable: false,
                sell_token_balance: SellTokenSource::Erc20,
                buy_token_balance: BuyTokenDestination::Erc20,
                price_quality: Default::default()
            }
        );
    }

    #[tokio::test]
    async fn post_quote_request_ok() {
        let filter = post_quote_request();
        let request_payload = OrderQuoteRequest::default();
        let request = request()
            .path("/quote")
            .method("POST")
            .header("content-type", "application/json")
            .json(&request_payload);
        let result = request.filter(&filter).await.unwrap();
        assert_eq!(result, request_payload);
    }

    #[tokio::test]
    async fn post_quote_request_err() {
        let filter = post_quote_request();
        let request_payload = OrderQuoteRequest::default();
        // Path is wrong!
        let request = request()
            .path("/fee_quote")
            .method("POST")
            .header("content-type", "application/json")
            .json(&request_payload);
        assert!(request.filter(&filter).await.is_err());
    }

    #[tokio::test]
    async fn post_quote_response_ok() {
        let quote = OrderQuote {
            sell_token: Default::default(),
            buy_token: Default::default(),
            receiver: None,
            sell_amount: Default::default(),
            buy_amount: Default::default(),
            valid_to: 0,
            app_data: Default::default(),
            fee_amount: Default::default(),
            kind: Default::default(),
            partially_fillable: false,
            sell_token_balance: Default::default(),
            buy_token_balance: Default::default(),
        };
        let order_quote_response = OrderQuoteResponse {
            quote,
            from: H160::zero(),
            expiration: DateTime::<Utc>::from_utc(NaiveDateTime::from_timestamp(0, 0), Utc),
        };
        let response = convert_json_response::<OrderQuoteResponse, OrderQuoteError>(Ok(
            order_quote_response.clone(),
        ))
        .into_response();
        assert_eq!(response.status(), StatusCode::OK);
        let body = response_body(response).await;
        let body: serde_json::Value = serde_json::from_slice(body.as_slice()).unwrap();
        let expected = serde_json::to_value(order_quote_response).unwrap();
        assert_eq!(body, expected);
    }

    #[tokio::test]
    async fn post_quote_response_err() {
        let response = convert_json_response::<OrderQuoteResponse, OrderQuoteError>(Err(
            OrderQuoteError::Order(ValidationError::Other(anyhow!("Uh oh - error"))),
        ))
        .into_response();
        assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
        let body = response_body(response).await;
        let body: serde_json::Value = serde_json::from_slice(body.as_slice()).unwrap();
        let expected_error = json!({"errorType": "InternalServerError", "description": ""});
        assert_eq!(body, expected_error);
        // There are many other FeeAndQuoteErrors, but writing a test for each would follow the same pattern as this.
    }

    #[test]
    fn calculate_fee_sell_before_fees_quote_request() {
        let mut fee_calculator = MockMinFeeCalculating::new();

        let expiration = Utc::now();
        fee_calculator
            .expect_compute_subsidized_min_fee()
            .returning(move |_, _| Ok((3.into(), expiration)));

        let fee_calculator = Arc::new(fee_calculator);
        let price_estimator = FakePriceEstimator(price_estimation::Estimate {
            out_amount: 14.into(),
            gas: 1000.into(),
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
            FeeParameters {
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
            .returning(move |_, _| Ok((3.into(), expiration)));

        let fee_calculator = Arc::new(fee_calculator);
        let price_estimator = FakePriceEstimator(price_estimation::Estimate {
            out_amount: 14.into(),
            gas: 1000.into(),
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
            FeeParameters {
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
            .returning(move |_, _| Ok((3.into(), expiration)));

        let fee_calculator = Arc::new(fee_calculator);
        let price_estimator = FakePriceEstimator(price_estimation::Estimate {
            out_amount: 20.into(),
            gas: 1000.into(),
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
            FeeParameters {
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
        let quote_request = OrderQuoteRequest::default();
        let result = PreOrderData::from(&quote_request);
        let expected = PreOrderData::default();
        assert_eq!(result, expected);
    }

    #[tokio::test]
    async fn calculate_quote() {
        let buy_request = OrderQuoteRequest {
            sell_token: H160::from_low_u64_be(1),
            buy_token: H160::from_low_u64_be(2),
            side: OrderQuoteSide::Buy {
                buy_amount_after_fee: 2.into(),
            },
            ..Default::default()
        };

        let mut fee_calculator = MockMinFeeCalculating::new();
        fee_calculator
            .expect_compute_subsidized_min_fee()
            .returning(move |_, _| Ok((3.into(), Utc::now())));
        let price_estimator = FakePriceEstimator(price_estimation::Estimate {
            out_amount: 14.into(),
            gas: 1000.into(),
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
