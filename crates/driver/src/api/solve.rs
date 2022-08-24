use crate::driver::Driver;
use anyhow::Result;
use model::auction::Auction;
use shared::api::{
    convert_json_response, error, extract_payload_with_max_size, ApiReply, IntoWarpReply,
};
use std::{convert::Infallible, sync::Arc};
use tracing::Instrument;
use warp::{hyper::StatusCode, reply::with_status, Filter, Rejection};

fn post_solve_request(
    prefix: &'static str,
) -> impl Filter<Extract = (Auction,), Error = Rejection> + Clone {
    warp::path(prefix)
        .and(warp::path("solve"))
        .and(warp::post())
        .and(extract_payload_with_max_size(1024 * 32))
}

pub fn post_solve(
    prefix: &'static str,
    driver: Arc<Driver>,
) -> impl Filter<Extract = (ApiReply,), Error = Rejection> + Clone {
    post_solve_request(prefix).and_then(move |auction: Auction| {
        let driver = driver.clone();
        async move {
            let result = driver
                .on_auction_started(auction.clone())
                .instrument(tracing::info_span!(
                    "auction",
                    id = auction.next_solver_competition
                ))
                .await;
            if let Err(err) = &result {
                tracing::warn!(?err, ?auction, "post_solve error");
            }
            Result::<_, Infallible>::Ok(convert_json_response(result))
        }
        .instrument(tracing::info_span!("solver", name = prefix))
    })
}

#[derive(thiserror::Error, Debug)]
pub enum SolveError {
    #[error("not implemented")]
    NotImplemented,
    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

impl IntoWarpReply for SolveError {
    fn into_warp_reply(self) -> ApiReply {
        match self {
            Self::NotImplemented => with_status(
                error("Route not yet implemented", "try again later"),
                StatusCode::INTERNAL_SERVER_ERROR,
            ),
            Self::Other(err) => err.into_warp_reply(),
        }
    }
}
