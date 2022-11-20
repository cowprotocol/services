use crate::orderbook::{OrderCancellationError, Orderbook};
use anyhow::Result;
use model::order::SignedOrderCancellations;
use shared::api::{convert_json_response, extract_payload};
use std::{convert::Infallible, sync::Arc};
use warp::{Filter, Rejection};

pub fn request() -> impl Filter<Extract = (SignedOrderCancellations,), Error = Rejection> + Clone {
    warp::path!("v1" / "orders")
        .and(warp::delete())
        .and(extract_payload())
}

pub fn response(result: Result<(), OrderCancellationError>) -> super::ApiReply {
    convert_json_response(result.map(|_| "Cancelled"))
}

pub fn filter(
    orderbook: Arc<Orderbook>,
) -> impl Filter<Extract = (super::ApiReply,), Error = Rejection> + Clone {
    request().and_then(move |cancellations| {
        let orderbook = orderbook.clone();
        async move {
            let result = orderbook.cancel_orders(cancellations).await;
            Result::<_, Infallible>::Ok(response(result))
        }
    })
}
