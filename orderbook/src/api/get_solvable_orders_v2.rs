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
            reply::json(&model::SolvableOrders {
                orders: orders.orders,
                latest_settlement_block: orders.latest_settlement_block,
            }),
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::response_body;
    use std::time::Instant;

    #[tokio::test]
    async fn serialize_response() {
        let orders = SolvableOrders {
            orders: vec![],
            update_time: Instant::now(),
            latest_settlement_block: 1,
        };
        let response = get_solvable_orders_response(Ok(orders)).into_response();
        assert_eq!(response.status(), StatusCode::OK);
        let body = response_body(response).await;
        let response: model::SolvableOrders = serde_json::from_slice(body.as_slice()).unwrap();
        assert_eq!(response.latest_settlement_block, 1);
    }
}
