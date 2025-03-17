use {
    crate::{
        api::ApiReply,
        orderbook::{self, Orderbook},
    },
    anyhow::Result,
    model::order::OrderUid,
    std::{convert::Infallible, sync::Arc},
    warp::{Filter, Rejection, hyper::StatusCode, reply},
};

fn get_status_request() -> impl Filter<Extract = (OrderUid,), Error = Rejection> + Clone {
    warp::path!("v1" / "orders" / OrderUid / "status").and(warp::get())
}

pub fn get_status(
    orderbook: Arc<Orderbook>,
) -> impl Filter<Extract = (ApiReply,), Error = Rejection> + Clone {
    get_status_request().and_then(move |uid| {
        let orderbook = orderbook.clone();
        async move {
            let status = orderbook.get_order_status(&uid).await;
            Result::<_, Infallible>::Ok(match status {
                Ok(status) => warp::reply::with_status(warp::reply::json(&status), StatusCode::OK),
                Err(orderbook::Error::NotFound) => reply::with_status(
                    super::error("NotFound", "Order status was not found"),
                    StatusCode::NOT_FOUND,
                ),
                Err(err) => {
                    tracing::error!(?err, "get_order_status");
                    *Box::new(crate::api::internal_error_reply())
                }
            })
        }
    })
}
