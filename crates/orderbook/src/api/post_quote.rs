use {
    super::post_order::{AppDataValidationErrorWrapper, PartialValidationErrorWrapper},
    crate::quoter::{OrderQuoteError, QuoteHandler},
    anyhow::Result,
    model::quote::OrderQuoteRequest,
    reqwest::StatusCode,
    shared::{
        api::{self, convert_json_response, rich_error, ApiReply, IntoWarpReply},
        order_quoting::CalculateQuoteError,
    },
    std::{convert::Infallible, sync::Arc},
    warp::{Filter, Rejection},
};

fn post_quote_request() -> impl Filter<Extract = (OrderQuoteRequest,), Error = Rejection> + Clone {
    warp::path!("v1" / "quote")
        .and(warp::post())
        .and(api::extract_payload())
}

pub fn post_quote(
    quotes: Arc<QuoteHandler>,
) -> impl Filter<Extract = (super::ApiReply,), Error = Rejection> + Clone {
    post_quote_request().and_then(move |request: OrderQuoteRequest| {
        let quotes = quotes.clone();
        async move {
            let result = quotes
                .calculate_quote(&request)
                .await
                .map_err(OrderQuoteErrorWrapper);
            if let Err(err) = &result {
                tracing::warn!(?err, ?request, "post_quote error");
            }
            Result::<_, Infallible>::Ok(convert_json_response(result))
        }
    })
}

#[derive(Debug)]
pub struct OrderQuoteErrorWrapper(pub OrderQuoteError);
impl IntoWarpReply for OrderQuoteErrorWrapper {
    fn into_warp_reply(self) -> ApiReply {
        match self.0 {
            OrderQuoteError::AppData(err) => AppDataValidationErrorWrapper(err).into_warp_reply(),
            OrderQuoteError::Order(err) => PartialValidationErrorWrapper(err).into_warp_reply(),
            OrderQuoteError::CalculateQuote(err) => {
                CalculateQuoteErrorWrapper(err).into_warp_reply()
            }
        }
    }
}

pub struct CalculateQuoteErrorWrapper(CalculateQuoteError);
impl IntoWarpReply for CalculateQuoteErrorWrapper {
    fn into_warp_reply(self) -> ApiReply {
        match self.0 {
            CalculateQuoteError::Price(err) => err.into_warp_reply(),
            CalculateQuoteError::SellAmountDoesNotCoverFee { fee_amount } => {
                warp::reply::with_status(
                    rich_error(
                        "SellAmountDoesNotCoverFee",
                        "The sell amount for the sell order is lower than the fee.",
                        serde_json::json!({ "fee_amount": fee_amount }),
                    ),
                    StatusCode::BAD_REQUEST,
                )
            }
            CalculateQuoteError::Other(err) => {
                tracing::error!(?err, "CalculateQuoteErrorWrapper");
                shared::api::internal_error_reply()
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        anyhow::anyhow,
        app_data::AppDataHash,
        chrono::{TimeZone, Utc},
        ethcontract::H160,
        model::{
            order::{BuyTokenDestination, SellTokenSource},
            quote::{
                OrderQuote,
                OrderQuoteResponse,
                OrderQuoteSide,
                PriceQuality,
                QuoteSigningScheme,
                SellAmount,
                Validity,
            },
        },
        number::nonzero::U256 as NonZeroU256,
        reqwest::StatusCode,
        serde_json::json,
        shared::{api::response_body, order_quoting::CalculateQuoteError},
        warp::{test::request, Reply},
    };

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
                "signingScheme": "presign",
                "priceQuality": "optimal"
            }))
            .unwrap(),
            OrderQuoteRequest {
                from: H160([0x01; 20]),
                sell_token: H160([0x02; 20]),
                buy_token: H160([0x03; 20]),
                receiver: None,
                side: OrderQuoteSide::Sell {
                    sell_amount: SellAmount::AfterFee {
                        value: NonZeroU256::try_from(1337).unwrap()
                    },
                },
                validity: Validity::To(0x12345678),
                app_data: AppDataHash([0x90; 32]).into(),
                sell_token_balance: SellTokenSource::Erc20,
                buy_token_balance: BuyTokenDestination::Internal,
                signing_scheme: QuoteSigningScheme::PreSign {
                    onchain_order: false
                },
                price_quality: PriceQuality::Optimal,
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
                "validFor": 1000,
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
                    sell_amount: SellAmount::BeforeFee {
                        value: NonZeroU256::try_from(1337).unwrap()
                    },
                },
                validity: Validity::For(1000),
                app_data: AppDataHash([0x90; 32]).into(),
                sell_token_balance: SellTokenSource::External,
                price_quality: PriceQuality::Fast,
                ..Default::default()
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
                    buy_amount_after_fee: NonZeroU256::try_from(1337).unwrap(),
                },
                validity: Validity::To(0x12345678),
                app_data: AppDataHash([0x90; 32]).into(),
                ..Default::default()
            }
        );
    }

    #[test]
    fn deserialize_minimum_parameters() {
        assert_eq!(
            serde_json::from_value::<OrderQuoteRequest>(json!({
                "from": "0x0101010101010101010101010101010101010101",
                "sellToken": "0x0202020202020202020202020202020202020202",
                "buyToken": "0x0303030303030303030303030303030303030303",
                "kind": "sell",
                "sellAmountAfterFee": "1337",
            }))
            .unwrap(),
            OrderQuoteRequest {
                from: H160([0x01; 20]),
                sell_token: H160([0x02; 20]),
                buy_token: H160([0x03; 20]),
                side: OrderQuoteSide::Sell {
                    sell_amount: SellAmount::AfterFee {
                        value: NonZeroU256::try_from(1337).unwrap()
                    },
                },
                ..Default::default()
            }
        );
    }

    #[tokio::test]
    async fn post_quote_request_ok() {
        let filter = post_quote_request();
        let request_payload = OrderQuoteRequest::default();
        let request = request()
            .path("/v1/quote")
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
            .path("/v1/fee_quote")
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
            signing_scheme: Default::default(),
        };
        let order_quote_response = OrderQuoteResponse {
            quote,
            from: H160::zero(),
            expiration: Utc.timestamp_millis_opt(0).unwrap(),
            id: Some(0),
            verified: false,
        };
        let response = convert_json_response::<OrderQuoteResponse, OrderQuoteErrorWrapper>(Ok(
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
        let response = convert_json_response::<OrderQuoteResponse, OrderQuoteErrorWrapper>(Err(
            OrderQuoteErrorWrapper(OrderQuoteError::CalculateQuote(CalculateQuoteError::Other(
                anyhow!("Uh oh - error"),
            ))),
        ))
        .into_response();
        assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
        let body = response_body(response).await;
        let body: serde_json::Value = serde_json::from_slice(body.as_slice()).unwrap();
        let expected_error = json!({"errorType": "InternalServerError", "description": ""});
        assert_eq!(body, expected_error);
        // There are many other FeeAndQuoteErrors, but writing a test for each
        // would follow the same pattern as this.
    }
}
