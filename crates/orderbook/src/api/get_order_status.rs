use {
    crate::orderbook::Orderbook,
    anyhow::Result,
    model::order::OrderUid,
    shared::api::ApiReply,
    std::{convert::Infallible, sync::Arc},
    warp::{hyper::StatusCode, Filter, Rejection},
};

fn get_status_request() -> impl Filter<Extract = (OrderUid,), Error = Rejection> + Clone {
    warp::path!("v1" / "status" / OrderUid).and(warp::get())
}

pub fn get_status(
    orderbook: Arc<Orderbook>,
) -> impl Filter<Extract = (ApiReply,), Error = Rejection> + Clone {
    get_status_request().and_then(move |order_uid| {
        let orderbook = orderbook.clone();
        async move {
            let status = orderbook.get_order_status(&order_uid).await;

            Result::<_, Infallible>::Ok(match status {
                Ok(status) => warp::reply::with_status(warp::reply::json(&status), StatusCode::OK),
                Err(err) => {
                    tracing::error!(?err, "get_order_status");
                    *Box::new(shared::api::internal_error_reply())
                }
            })
        }
    })
}
