use super::H160Wrapper;
use crate::api::convert_get_trades_error_to_reply;
use crate::database::{Database, TradeFilter};
use anyhow::Result;
use futures::TryStreamExt;
use model::order::OrderUid;
use model::trade::Trade;
use serde::Deserialize;
use std::convert::Infallible;
use std::sync::Arc;
use warp::reply::{Json, WithStatus};
use warp::{hyper::StatusCode, Filter, Rejection, Reply};

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Query {
    pub order_uid: Option<OrderUid>,
    pub owner: Option<H160Wrapper>,
}

#[derive(Debug, Eq, PartialEq)]
enum TradeFilterError {
    InvalidFilter(String),
}

impl Query {
    fn trade_filter(&self) -> TradeFilter {
        let to_h160 = |option: Option<&H160Wrapper>| option.map(|wrapper| wrapper.0);
        TradeFilter {
            order_uid: self.order_uid,
            owner: to_h160(self.owner.as_ref()),
        }
    }

    fn validate(&self) -> Result<TradeFilter, TradeFilterError> {
        // Ensure that not both owner and order_uid are specified
        if self.order_uid.is_some() && self.owner.is_some() {
            return Err(TradeFilterError::InvalidFilter(
                "Cannot specify both owner and order_uid".to_owned(),
            ));
        }
        Ok(self.trade_filter())
    }
}

fn get_trades_request(
) -> impl Filter<Extract = (Result<TradeFilter, TradeFilterError>,), Error = Rejection> + Clone {
    warp::path!("trades")
        .and(warp::get())
        .and(warp::query::<Query>())
        .map(|query: Query| query.validate())
}

fn get_trades_response(result: Result<Vec<Trade>>) -> WithStatus<Json> {
    match result {
        Ok(trades) => warp::reply::with_status(warp::reply::json(&trades), StatusCode::OK),
        Err(err) => convert_get_trades_error_to_reply(err),
    }
}

pub fn get_trades(
    db: Arc<Database>,
) -> impl Filter<Extract = (impl Reply,), Error = Rejection> + Clone {
    get_trades_request().and_then(move |request_result| {
        let database = db.clone();
        async move {
            match request_result {
                Ok(trade_filter) => {
                    let result = database.trades(&trade_filter).try_collect::<Vec<_>>().await;
                    Result::<_, Infallible>::Ok(get_trades_response(result))
                }
                Err(TradeFilterError::InvalidFilter(msg)) => {
                    let err = super::error("InvalidTradeFilter", msg);
                    Ok(warp::reply::with_status(err, StatusCode::BAD_REQUEST))
                }
            }
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::get_trades::TradeFilterError::InvalidFilter;
    use crate::api::response_body;
    use hex_literal::hex;
    use primitive_types::H160;
    use warp::test::{request, RequestBuilder};

    #[tokio::test]
    async fn get_trades_request_ok() {
        let trade_filter = |request: RequestBuilder| async move {
            let filter = get_trades_request();
            request.method("GET").filter(&filter).await
        };
        let result = trade_filter(request().path("/trades"))
            .await
            .unwrap()
            .unwrap();
        assert_eq!(result.owner, None);
        assert_eq!(result.order_uid, None);

        let owner = H160::from_slice(&hex!("0000000000000000000000000000000000000001"));
        let owner_path = format!("/trades?owner=0x{:x}", owner);

        let result = trade_filter(request().path(owner_path.as_str()))
            .await
            .unwrap()
            .unwrap();
        assert_eq!(result.owner, Some(owner));
        assert_eq!(result.order_uid, None);

        let uid = OrderUid([1u8; 56]);
        let order_uid_path = format!("/trades?orderUid={:}", uid);
        let result = trade_filter(request().path(order_uid_path.as_str()))
            .await
            .unwrap()
            .unwrap();
        assert_eq!(result.owner, None);
        assert_eq!(result.order_uid, Some(uid));
    }

    #[tokio::test]
    async fn get_trades_request_err() {
        let trade_filter = |request: RequestBuilder| async move {
            let filter = get_trades_request();
            request.method("GET").filter(&filter).await
        };

        let owner = H160::from_slice(&hex!("0000000000000000000000000000000000000001"));
        let uid = OrderUid([1u8; 56]);
        let path = format!("/trades?owner=0x{:x}&orderUid={:}", owner, uid);

        let result = trade_filter(request().path(path.as_str())).await.unwrap();
        let expected = Err(InvalidFilter(
            "Cannot specify both owner and order_uid".to_owned(),
        ));
        assert_eq!(result, expected);
    }

    #[tokio::test]
    async fn get_trades_response_ok() {
        let trades = vec![Trade::default()];
        let response = get_trades_response(Ok(trades.clone())).into_response();
        assert_eq!(response.status(), StatusCode::OK);
        let body = response_body(response).await;
        let response_trades: Vec<Trade> = serde_json::from_slice(body.as_slice()).unwrap();
        assert_eq!(response_trades, trades);
    }
}
