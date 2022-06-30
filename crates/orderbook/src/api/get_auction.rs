use crate::orderbook::Orderbook;
use anyhow::Result;
use shared::api::{convert_json_response, ApiReply};
use std::{convert::Infallible, sync::Arc};
use warp::{Filter, Rejection};

fn get_auction_request() -> impl Filter<Extract = (), Error = Rejection> + Clone {
    warp::path!("auction").and(warp::get())
}

pub fn get_auction(
    orderbook: Arc<Orderbook>,
) -> impl Filter<Extract = (ApiReply,), Error = Rejection> + Clone {
    get_auction_request().and_then(move || {
        let orderbook = orderbook.clone();
        async move {
            let auction = orderbook.get_auction();
            Result::<_, Infallible>::Ok(convert_json_response(auction))
        }
    })
}
