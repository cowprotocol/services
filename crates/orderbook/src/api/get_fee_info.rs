use super::post_quote::OrderQuoteErrorWrapper;
use anyhow::Result;
use chrono::{DateTime, Utc};
use model::{
    order::OrderKind,
    quote::{OrderQuoteRequest, OrderQuoteSide, SellAmount},
    u256_decimal,
};
use primitive_types::{H160, U256};
use serde::{Deserialize, Serialize};
use shared::{
    api::{convert_json_response, ApiReply},
    order_quoting::QuoteHandler,
};
use std::{convert::Infallible, sync::Arc};
use warp::{Filter, Rejection};

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct FeeInfo {
    pub expiration_date: DateTime<Utc>,
    #[serde(with = "u256_decimal")]
    pub amount: U256,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Query {
    sell_token: H160,
    buy_token: H160,
    #[serde(with = "u256_decimal")]
    amount: U256,
    kind: OrderKind,
}

fn get_fee_info_request() -> impl Filter<Extract = (Query,), Error = Rejection> + Clone {
    warp::path!("fee")
        .and(warp::get())
        .and(warp::query::<Query>())
}

pub fn get_fee_info(
    quotes: Arc<QuoteHandler>,
) -> impl Filter<Extract = (ApiReply,), Error = Rejection> + Clone {
    get_fee_info_request().and_then(move |query: Query| {
        let quotes = quotes.clone();
        async move {
            let response = quotes
                .calculate_quote(&OrderQuoteRequest {
                    sell_token: query.sell_token,
                    buy_token: query.buy_token,
                    side: match query.kind {
                        OrderKind::Buy => OrderQuoteSide::Buy {
                            buy_amount_after_fee: query.amount,
                        },
                        OrderKind::Sell => OrderQuoteSide::Sell {
                            sell_amount: SellAmount::AfterFee {
                                value: query.amount,
                            },
                        },
                    },
                    ..Default::default()
                })
                .await
                .map_err(OrderQuoteErrorWrapper);
            Result::<_, Infallible>::Ok(convert_json_response(response.map(|response| FeeInfo {
                expiration_date: response.expiration,
                amount: response.quote.fee_amount,
            })))
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::FixedOffset;
    use shared::{api::response_body, price_estimation::PriceEstimationError};
    use warp::{hyper::StatusCode, test::request, Reply};

    #[tokio::test]
    async fn get_fee_info_request_ok() {
        let filter = get_fee_info_request();
        let sell_token = String::from("0x0000000000000000000000000000000000000001");
        let buy_token = String::from("0x0000000000000000000000000000000000000002");
        let path_string = format!(
            "/fee?sellToken={}&buyToken={}&amount={}&kind=buy",
            sell_token,
            buy_token,
            U256::exp10(18)
        );
        let request = request().path(&path_string).method("GET");
        let result = request.filter(&filter).await.unwrap();
        assert_eq!(result.sell_token, H160::from_low_u64_be(1));
        assert_eq!(result.buy_token, H160::from_low_u64_be(2));
        assert_eq!(result.amount, U256::exp10(18));
        assert_eq!(result.kind, OrderKind::Buy);
    }

    #[tokio::test]
    async fn get_fee_info_response_() {
        let result = Ok(FeeInfo {
            expiration_date: Utc::now() + FixedOffset::east(10),
            amount: U256::zero(),
        });
        let response = convert_json_response::<_, PriceEstimationError>(result).into_response();
        assert_eq!(response.status(), StatusCode::OK);
        let body = response_body(response).await;
        let body: FeeInfo = serde_json::from_slice(body.as_slice()).unwrap();
        assert_eq!(body.amount, U256::zero());
        assert!(body.expiration_date.gt(&chrono::offset::Utc::now()))
    }
}
