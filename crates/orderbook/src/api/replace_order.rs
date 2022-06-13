use crate::{
    api::{extract_payload, IntoWarpReply},
    orderbook::{Orderbook, ReplaceOrderError},
};
use anyhow::Result;
use model::order::{OrderCreation, OrderUid};
use reqwest::StatusCode;
use std::{convert::Infallible, sync::Arc};
use warp::{reply, Filter, Rejection};

fn request() -> impl Filter<Extract = (OrderUid, OrderCreation), Error = Rejection> + Clone {
    warp::path!("orders" / OrderUid)
        .and(warp::patch())
        .and(extract_payload())
}

fn response(result: Result<OrderUid, ReplaceOrderError>) -> super::ApiReply {
    match result {
        Ok(response) => reply::with_status(reply::json(&response), StatusCode::CREATED),
        Err(err) => err.into_warp_reply(),
    }
}

pub fn filter(
    orderbook: Arc<Orderbook>,
) -> impl Filter<Extract = (super::ApiReply,), Error = Rejection> + Clone {
    request().and_then(move |old_order, new_order| {
        let orderbook = orderbook.clone();
        async move {
            let result = orderbook.replace_order(old_order, new_order).await;
            Result::<_, Infallible>::Ok(response(result))
        }
    })
}

impl IntoWarpReply for ReplaceOrderError {
    fn into_warp_reply(self) -> super::ApiReply {
        match self {
            ReplaceOrderError::Cancellation(err) => err.into_warp_reply(),
            ReplaceOrderError::Add(err) => err.into_warp_reply(),
            err @ ReplaceOrderError::InvalidReplacement => reply::with_status(
                super::error("InvalidReplacement", err.to_string()),
                StatusCode::UNAUTHORIZED,
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn replace_order_request_filter() {
        let old_order = OrderUid::default();
        let new_order = OrderCreation::default();

        let result = warp::test::request()
            .path(&format!("/orders/{old_order}"))
            .method("PATCH")
            .header("content-type", "application/json")
            .json(&new_order)
            .filter(&request())
            .await
            .unwrap();

        assert_eq!(result, (old_order, new_order));
    }
}
