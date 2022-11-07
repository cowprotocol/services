use crate::orderbook::{OrderCancellationError, Orderbook};
use anyhow::Result;
use model::{
    order::SignedOrderCancellations,
    signature::{EcdsaSignature, EcdsaSigningScheme},
};
use serde::{Deserialize, Serialize};
use shared::api::{convert_json_response, extract_payload};
use std::{convert::Infallible, sync::Arc};
use warp::{Filter, Rejection};

#[derive(Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
struct CancellationPayload {
    signature: EcdsaSignature,
    signing_scheme: EcdsaSigningScheme,
}

pub fn request() -> impl Filter<Extract = (SignedOrderCancellations,), Error = Rejection> + Clone {
    warp::path!("orders")
        .and(warp::delete())
        .and(extract_payload())
}

pub fn response(result: Result<(), OrderCancellationError>) -> super::ApiReply {
    convert_json_response(result.map(|_| "Cancelled"))
}

pub fn delete(
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
