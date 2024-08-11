use {
    super::with_status,
    crate::database::trades::{TradeFilter, TradeRetrieving},
    anyhow::{Context, Result},
    axum::{http::StatusCode, routing::MethodRouter},
    model::order::OrderUid,
    primitive_types::H160,
    serde::Deserialize,
    shared::api::{error, internal_error_reply, ApiReply},
};

pub fn route() -> (&'static str, MethodRouter<super::State>) {
    (ENDPOINT, axum::routing::get(handler))
}

const ENDPOINT: &str = "/api/v1/trades";
async fn handler(
    state: axum::extract::State<super::State>,
    query: axum::extract::Query<Query>,
) -> ApiReply {
    match query.trade_filter() {
        Ok(trade_filter) => {
            let result = state
                .database
                .trades(&trade_filter)
                .await
                .context("get_trades");

            match result {
                Ok(reply) => with_status(serde_json::to_value(&reply).unwrap(), StatusCode::OK),
                Err(err) => {
                    tracing::error!(?err, "get_trades");
                    internal_error_reply()
                }
            }
        }
        Err(TradeFilterError::InvalidFilter(msg)) => {
            with_status(error("InvalidTradeFilter", msg), StatusCode::BAD_REQUEST)
        }
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Query {
    pub order_uid: Option<OrderUid>,
    pub owner: Option<H160>,
}

#[derive(Debug, Eq, PartialEq)]
enum TradeFilterError {
    InvalidFilter(String),
}

impl Query {
    fn trade_filter(&self) -> Result<TradeFilter, TradeFilterError> {
        match (self.order_uid.as_ref(), self.owner.as_ref()) {
            (Some(_), None) | (None, Some(_)) => Ok(TradeFilter {
                order_uid: self.order_uid,
                owner: self.owner,
            }),
            _ => Err(TradeFilterError::InvalidFilter(
                "Must specify exactly one of owner and order_uid.".to_owned(),
            )),
        }
    }
}

// #[cfg(test)]
// mod tests {
//     use {
//         super::*,
//         hex_literal::hex,
//         primitive_types::H160,
//         warp::test::{request, RequestBuilder},
//     };

//     #[tokio::test]
//     async fn get_trades_request_ok() {
//         let trade_filter = |request: RequestBuilder| async move {
//             let filter = get_trades_request();
//             request.method("GET").filter(&filter).await
//         };

//         let owner =
// H160::from_slice(&hex!("0000000000000000000000000000000000000001"));
//         let owner_path = format!("/v1/trades?owner=0x{owner:x}");
//         let result = trade_filter(request().path(owner_path.as_str()))
//             .await
//             .unwrap()
//             .unwrap();
//         assert_eq!(result.owner, Some(owner));
//         assert_eq!(result.order_uid, None);

//         let uid = OrderUid([1u8; 56]);
//         let order_uid_path = format!("/v1/trades?orderUid={uid}");
//         let result = trade_filter(request().path(order_uid_path.as_str()))
//             .await
//             .unwrap()
//             .unwrap();
//         assert_eq!(result.owner, None);
//         assert_eq!(result.order_uid, Some(uid));
//     }

//     #[tokio::test]
//     async fn get_trades_request_err() {
//         let trade_filter = |request: RequestBuilder| async move {
//             let filter = get_trades_request();
//             request.method("GET").filter(&filter).await
//         };

//         let owner =
// H160::from_slice(&hex!("0000000000000000000000000000000000000001"));
//         let uid = OrderUid([1u8; 56]);
//         let path = format!("/v1/trades?owner=0x{owner:x}&orderUid={uid}");

//         let result =
// trade_filter(request().path(path.as_str())).await.unwrap();         assert!
// (result.is_err());

//         let path = "/v1/trades";
//         let result = trade_filter(request().path(path)).await.unwrap();
//         assert!(result.is_err());
//     }
// }
