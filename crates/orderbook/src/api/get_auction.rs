use super::IntoWarpReply;
use anyhow::anyhow;
use std::convert::Infallible;
use warp::{Filter, Rejection};

fn get_auction_request() -> impl Filter<Extract = (), Error = Rejection> + Clone {
    warp::path!("auction").and(warp::get())
}

pub fn get_auction() -> impl Filter<Extract = (super::ApiReply,), Error = Rejection> + Clone {
    get_auction_request().and_then(move || async move {
        Result::<_, Infallible>::Ok(anyhow!("not yet implemented").into_warp_reply())
    })
}
