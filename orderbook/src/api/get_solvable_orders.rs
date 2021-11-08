use crate::{api::convert_json_response, orderbook::Orderbook};
use anyhow::Result;
use std::{convert::Infallible, sync::Arc};
use warp::{Filter, Rejection};

fn get_solvable_orders_request() -> impl Filter<Extract = (), Error = Rejection> + Clone {
    warp::path!("solvable_orders").and(warp::get())
}

pub fn get_solvable_orders(
    orderbook: Arc<Orderbook>,
) -> impl Filter<Extract = (super::ApiReply,), Error = Rejection> + Clone {
    get_solvable_orders_request().and_then(move || {
        let orderbook = orderbook.clone();
        async move {
            let result = orderbook.get_solvable_orders().await;
            Result::<_, Infallible>::Ok(convert_json_response(
                result.map(|solvable_orders| solvable_orders.orders),
            ))
        }
    })
}
