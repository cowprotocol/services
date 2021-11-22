use crate::fee::MinFeeCalculating;
use crate::{api::convert_json_response, fee::FeeData};
use anyhow::Result;
use chrono::{DateTime, Utc};
use model::{order::OrderKind, u256_decimal};
use primitive_types::{H160, U256};
use serde::{Deserialize, Serialize};
use std::convert::Infallible;
use std::sync::Arc;
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
    fee_calculator: Arc<dyn MinFeeCalculating>,
) -> impl Filter<Extract = (super::ApiReply,), Error = Rejection> + Clone {
    get_fee_info_request().and_then(move |query: Query| {
        let fee_calculator = fee_calculator.clone();
        async move {
            let result = fee_calculator
                .compute_subsidized_min_fee(
                    FeeData {
                        sell_token: query.sell_token,
                        buy_token: query.buy_token,
                        amount: query.amount,
                        kind: query.kind,
                    },
                    Default::default(),
                )
                .await;
            Result::<_, Infallible>::Ok(convert_json_response(result.map(
                |(amount, expiration_date)| FeeInfo {
                    expiration_date,
                    amount,
                },
            )))
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::response_body;
    use chrono::FixedOffset;
    use shared::price_estimation::PriceEstimationError;
    use warp::hyper::StatusCode;
    use warp::{test::request, Reply};

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
