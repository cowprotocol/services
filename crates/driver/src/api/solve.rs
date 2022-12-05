use {
    crate::driver::Driver,
    anyhow::Result,
    model::auction::AuctionWithId,
    shared::api::{
        convert_json_response,
        error,
        extract_payload_with_max_size,
        ApiReply,
        IntoWarpReply,
    },
    std::{convert::Infallible, sync::Arc},
    tracing::Instrument,
    warp::{hyper::StatusCode, reply::with_status, Filter, Rejection},
};

fn post_solve_request(
    prefix: &'static str,
) -> impl Filter<Extract = (AuctionWithId,), Error = Rejection> + Clone {
    warp::path(prefix)
        .and(warp::path("solve"))
        .and(warp::post())
        .and(extract_payload_with_max_size(1024 * 32))
}

pub fn post_solve(
    prefix: &'static str,
    driver: Arc<Driver>,
) -> impl Filter<Extract = (ApiReply,), Error = Rejection> + Clone {
    post_solve_request(prefix).and_then(move |auction: AuctionWithId| {
        let driver = driver.clone();
        let auction_id = auction.id;
        async move {
            let result = driver.on_auction_started(auction.clone()).await;
            if let Err(err) = &result {
                tracing::warn!(?err, "post_solve error");
            }
            Result::<_, Infallible>::Ok(convert_json_response(result))
        }
        .instrument(tracing::info_span!("solve", solver = prefix, auction_id))
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
            Self::Other(err) => with_status(
                error("InternalServerError", err.to_string()),
                StatusCode::INTERNAL_SERVER_ERROR,
            ),
        }
    }
}
