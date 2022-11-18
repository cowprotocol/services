use crate::orderbook::Orderbook;
use anyhow::Result;
use shared::api::{convert_json_response, ApiReply};
use std::{convert::Infallible, sync::Arc};
use warp::{Filter, Rejection};

fn get_solvable_orders_request() -> impl Filter<Extract = (), Error = Rejection> + Clone {
    warp::path!("v1" / "solvable_orders").and(warp::get())
}

pub fn get_solvable_orders(
    orderbook: Arc<Orderbook>,
) -> impl Filter<Extract = (ApiReply,), Error = Rejection> + Clone {
    get_solvable_orders_request().and_then(move || {
        let orderbook = orderbook.clone();
        async move {
            let result = orderbook.get_auction().await;
            Result::<_, Infallible>::Ok(convert_json_response(result.map(|auction| {
                auction
                    .map(|auction| auction.auction.orders)
                    .unwrap_or_default()
            })))
        }
    })
}
