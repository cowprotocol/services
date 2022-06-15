use crate::{
    api::{self, convert_json_response, IntoWarpReply},
    order_quoting::{FeeError, OrderQuoteError, OrderQuoter},
};
use anyhow::Result;
use model::quote::OrderQuoteRequest;
use std::{convert::Infallible, sync::Arc};
use warp::{hyper::StatusCode, Filter, Rejection};

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

impl IntoWarpReply for FeeError {
    fn into_warp_reply(self) -> super::ApiReply {
        match self {
            FeeError::PriceEstimate(err) => err.into_warp_reply(),
            FeeError::SellAmountDoesNotCoverFee(fee) => warp::reply::with_status(
                super::rich_error(
                    "SellAmountDoesNotCoverFee",
                    "The sell amount for the sell order is lower than the fee.",
                    fee,
                ),
                StatusCode::BAD_REQUEST,
            ),
        }
    }
}

impl IntoWarpReply for OrderQuoteError {
    fn into_warp_reply(self) -> super::ApiReply {
        match self {
            OrderQuoteError::Fee(err) => err.into_warp_reply(),
            OrderQuoteError::Order(err) => err.into_warp_reply(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{api::response_body, order_validation::ValidationError};
    use anyhow::anyhow;
    use chrono::{DateTime, NaiveDateTime, Utc};
    use ethcontract::{H160, U256};
    use model::{
        app_id::AppId,
        order::{BuyTokenDestination, SellTokenSource},
        quote::{
            OrderQuote, OrderQuoteResponse, OrderQuoteSide, PriceQuality, SellAmount, Validity,
        },
        signature::SigningScheme,
    };
    use serde_json::json;
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
                    sell_amount: SellAmount::AfterFee { value: 1337.into() },
                },
                validity: Validity::To(0x12345678),
                app_data: AppId([0x90; 32]),
                partially_fillable: false,
                sell_token_balance: SellTokenSource::Erc20,
                buy_token_balance: BuyTokenDestination::Internal,
                signing_scheme: SigningScheme::PreSign,
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
                    sell_amount: SellAmount::BeforeFee { value: 1337.into() },
                },
                validity: Validity::For(1000),
                app_data: AppId([0x90; 32]),
                partially_fillable: false,
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
                    buy_amount_after_fee: U256::from(1337),
                },
                validity: Validity::To(0x12345678),
                app_data: AppId([0x90; 32]),
                partially_fillable: false,
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
                    sell_amount: SellAmount::AfterFee { value: 1337.into() },
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
            id: 0,
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
}
