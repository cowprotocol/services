use crate::{commit_reveal::SettlementSummary, driver::Driver};
use anyhow::Result;
use shared::api::{convert_json_response, error, extract_payload, ApiReply, IntoWarpReply};
use std::{convert::Infallible, sync::Arc};
use tracing::Instrument;
use warp::{hyper::StatusCode, reply::with_status, Filter, Rejection};

fn post_execute_request(
    prefix: &'static str,
) -> impl Filter<Extract = (SettlementSummary,), Error = Rejection> + Clone {
    warp::path(prefix)
        .and(warp::path("execute"))
        .and(warp::post())
        .and(extract_payload())
}

pub fn post_execute(
    prefix: &'static str,
    driver: Arc<Driver>,
) -> impl Filter<Extract = (ApiReply,), Error = Rejection> + Clone {
    post_execute_request(prefix).and_then(move |summary: SettlementSummary| {
        let driver = driver.clone();
        let auction_id = summary.auction_id;
        let settlement_id = summary.settlement_id;
        async move {
            let result = driver.on_auction_won(summary.clone()).await;
            if let Err(err) = &result {
                tracing::warn!(?err, "post_execute error");
            }
            Result::<_, Infallible>::Ok(convert_json_response(result))
        }
        .instrument(tracing::info_span!(
            "execute",
            solver = prefix,
            auction_id,
            settlement_id
        ))
    })
}

#[derive(thiserror::Error, Debug)]
pub enum ExecuteError {
    #[error("settlement execution rejected")]
    ExecutionRejected,
    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

impl IntoWarpReply for ExecuteError {
    fn into_warp_reply(self) -> ApiReply {
        match self {
            Self::ExecutionRejected => with_status(
                error(
                    "ExecutionRejected",
                    "the solver no longer wants to execute the settlement",
                ),
                StatusCode::INTERNAL_SERVER_ERROR,
            ),
            Self::Other(err) => with_status(
                error("InternalServerError", err.to_string()),
                StatusCode::INTERNAL_SERVER_ERROR,
            ),
        }
    }
}
