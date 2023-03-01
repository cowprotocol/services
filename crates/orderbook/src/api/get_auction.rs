use {
    crate::orderbook::Orderbook,
    anyhow::Result,
    reqwest::StatusCode,
    shared::api::ApiReply,
    std::{convert::Infallible, sync::Arc},
    warp::{reply::with_status, Filter, Rejection},
};

fn get_auction_request() -> impl Filter<Extract = (), Error = Rejection> + Clone {
    warp::path!("v1" / "auction").and(warp::get())
}

pub fn get_auction(
    orderbook: Arc<Orderbook>,
) -> impl Filter<Extract = (ApiReply,), Error = Rejection> + Clone {
    get_auction_request().and_then(move || {
        let orderbook = orderbook.clone();
        async move {
            let result = orderbook.get_auction().await;
            let reply = match result {
                Ok(Some(auction)) => with_status(warp::reply::json(&auction), StatusCode::OK),
                Ok(None) => with_status(
                    super::error("NotFound", "There is no active auction"),
                    StatusCode::NOT_FOUND,
                ),
                Err(err) => {
                    tracing::error!(?err, "/api/v1/get_auction");
                    shared::api::internal_error_reply()
                }
            };
            Result::<_, Infallible>::Ok(reply)
        }
    })
}
