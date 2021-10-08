use crate::orderbook::Orderbook;
use crate::{api::convert_get_orders_error_to_reply, solvable_orders::SolvableOrders};
use anyhow::Result;
use std::{convert::Infallible, sync::Arc};
use warp::{hyper::StatusCode, reply, Filter, Rejection, Reply};

fn get_solvable_orders_request() -> impl Filter<Extract = (), Error = Rejection> + Clone {
    warp::path!("solvable_orders").and(warp::get())
}

fn get_solvable_orders_response(result: Result<SolvableOrders>) -> impl Reply {
    match result {
        Ok(orders) => Ok(reply::with_status(
            reply::json(&orders.orders),
            StatusCode::OK,
        )),
        Err(err) => Ok(convert_get_orders_error_to_reply(err)),
    }
}

pub fn get_solvable_orders(
    orderbook: Arc<Orderbook>,
) -> impl Filter<Extract = (impl Reply,), Error = Rejection> + Clone {
    get_solvable_orders_request().and_then(move || {
        let orderbook = orderbook.clone();
        async move {
            let result = orderbook.get_solvable_orders().await;
            Result::<_, Infallible>::Ok(get_solvable_orders_response(result))
        }
    })
}
