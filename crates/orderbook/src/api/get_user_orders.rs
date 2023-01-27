use crate::orderbook::Orderbook;
use anyhow::Result;
use primitive_types::H160;
use serde::Deserialize;
use shared::api::ApiReply;
use std::{convert::Infallible, sync::Arc};
use warp::{hyper::StatusCode, reply::with_status, Filter, Rejection};

#[derive(Clone, Copy, Debug, Deserialize)]
struct Query {
    offset: Option<u64>,
    limit: Option<u64>,
}

fn request() -> impl Filter<Extract = (H160, Query), Error = Rejection> + Clone {
    warp::path!("v1" / "account" / H160 / "orders")
        .and(warp::get())
        .and(warp::query::<Query>())
}

pub fn get_user_orders(
    orderbook: Arc<Orderbook>,
) -> impl Filter<Extract = (ApiReply,), Error = Rejection> + Clone {
    request().and_then(move |owner: H160, query: Query| {
        let orderbook = orderbook.clone();
        async move {
            const DEFAULT_OFFSET: u64 = 0;
            const DEFAULT_LIMIT: u64 = 10;
            const MIN_LIMIT: u64 = 1;
            const MAX_LIMIT: u64 = 1000;
            let offset = query.offset.unwrap_or(DEFAULT_OFFSET);
            let limit = query.limit.unwrap_or(DEFAULT_LIMIT);
            if !(MIN_LIMIT..=MAX_LIMIT).contains(&limit) {
                return Ok(with_status(
                    super::error(
                        "LIMIT_OUT_OF_BOUNDS",
                        format!("The pagination limit is [{MIN_LIMIT},{MAX_LIMIT}]."),
                    ),
                    StatusCode::BAD_REQUEST,
                ));
            }
            let result = orderbook.get_user_orders(&owner, offset, limit).await;
            Result::<_, Infallible>::Ok(match result {
                Ok(reply) => with_status(warp::reply::json(&reply), StatusCode::OK),
                Err(err) => {
                    tracing::error!(?err, "get_user_orders");
                    shared::api::internal_error_reply()
                }
            })
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use shared::addr;

    #[tokio::test]
    async fn request_() {
        let path = "/v1/account/0x0000000000000000000000000000000000000001/orders";
        let result = warp::test::request()
            .path(path)
            .method("GET")
            .filter(&request())
            .await
            .unwrap();
        assert_eq!(result.0, addr!("0000000000000000000000000000000000000001"));
        assert_eq!(result.1.offset, None);
        assert_eq!(result.1.limit, None);

        let path = "/v1/account/0x0000000000000000000000000000000000000001/orders?offset=1&limit=2";
        let result = warp::test::request()
            .path(path)
            .method("GET")
            .filter(&request())
            .await
            .unwrap();
        assert_eq!(result.1.offset, Some(1));
        assert_eq!(result.1.limit, Some(2));
    }
}
