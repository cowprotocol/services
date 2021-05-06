use crate::fee::{MinFeeCalculationError, MinFeeCalculator};

use super::H160Wrapper;
use anyhow::Result;
use chrono::{DateTime, Utc};
use model::{order::OrderKind, u256_decimal};
use primitive_types::U256;
use serde::{Deserialize, Serialize};
use std::convert::Infallible;
use std::sync::Arc;
use warp::{hyper::StatusCode, reply, Filter, Rejection, Reply};

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
    sell_token: H160Wrapper,
    buy_token: H160Wrapper,
    #[serde(with = "u256_decimal")]
    amount: U256,
    kind: OrderKind,
}

fn get_fee_info_request() -> impl Filter<Extract = (Query,), Error = Rejection> + Clone {
    warp::path!("fee")
        .and(warp::get())
        .and(warp::query::<Query>())
}

pub fn get_fee_info_response(
    result: Result<(U256, DateTime<Utc>), MinFeeCalculationError>,
) -> impl Reply {
    match result {
        Ok((amount, expiration_date)) => {
            let fee_info = FeeInfo {
                expiration_date,
                amount,
            };
            Ok(reply::with_status(reply::json(&fee_info), StatusCode::OK))
        }
        Err(MinFeeCalculationError::NotFound) => Ok(reply::with_status(
            super::error("NotFound", "Token was not found"),
            StatusCode::NOT_FOUND,
        )),
        Err(MinFeeCalculationError::UnsupportedToken(token)) => Ok(reply::with_status(
            super::error("UnsupportedToken", format!("Token address {:?}", token)),
            StatusCode::BAD_REQUEST,
        )),
        Err(MinFeeCalculationError::Other(err)) => {
            tracing::error!(?err, "get_fee error");
            Ok(reply::with_status(
                super::internal_error(),
                StatusCode::INTERNAL_SERVER_ERROR,
            ))
        }
    }
}

pub fn get_fee_info(
    fee_calculator: Arc<MinFeeCalculator>,
) -> impl Filter<Extract = (impl Reply,), Error = Rejection> + Clone {
    get_fee_info_request().and_then(move |query: Query| {
        let fee_calculator = fee_calculator.clone();
        async move {
            Result::<_, Infallible>::Ok(get_fee_info_response(
                fee_calculator
                    .min_fee(
                        query.sell_token.0,
                        query.buy_token.0,
                        query.amount,
                        query.kind,
                    )
                    .await,
            ))
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::response_body;
    use chrono::FixedOffset;
    use primitive_types::H160;
    use warp::test::request;

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
        assert_eq!(result.sell_token.0, H160::from_low_u64_be(1));
        assert_eq!(result.buy_token.0, H160::from_low_u64_be(2));
        assert_eq!(result.amount, U256::exp10(18));
        assert_eq!(result.kind, OrderKind::Buy);
    }

    #[tokio::test]
    async fn get_fee_info_response_() {
        let response =
            get_fee_info_response(Ok((U256::zero(), Utc::now() + FixedOffset::east(10))))
                .into_response();
        assert_eq!(response.status(), StatusCode::OK);
        let body = response_body(response).await;
        let body: FeeInfo = serde_json::from_slice(body.as_slice()).unwrap();
        assert_eq!(body.amount, U256::zero());
        assert!(body.expiration_date.gt(&chrono::offset::Utc::now()))
    }
}
