use anyhow::Result;
use shared::api::{convert_json_response, error, extract_payload, ApiReply, IntoWarpReply};
use std::convert::Infallible;
use warp::{hyper::StatusCode, reply::with_status, Filter, Rejection};

fn post_execute_request() -> impl Filter<Extract = (String,), Error = Rejection> + Clone {
    warp::path!("execute")
        .and(warp::post())
        .and(extract_payload())
}

pub fn post_execute() -> impl Filter<Extract = (ApiReply,), Error = Rejection> + Clone {
    post_execute_request().and_then(move |request| async move {
        let result: Result<(), _> = Err(ExecuteError::NotImplemented);
        if let Err(err) = &result {
            tracing::warn!(?err, ?request, "post_execute error");
        }
        Result::<_, Infallible>::Ok(convert_json_response(result))
    })
}

#[derive(thiserror::Error, Debug)]
pub enum ExecuteError {
    #[error("not implemented")]
    NotImplemented,
    #[error("settlement execution rejected")]
    ExecutionRejected,
    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

impl IntoWarpReply for ExecuteError {
    fn into_warp_reply(self) -> ApiReply {
        match self {
            Self::NotImplemented => with_status(
                error("Route not yet implemented", "try again later"),
                StatusCode::INTERNAL_SERVER_ERROR,
            ),
            Self::ExecutionRejected => with_status(
                error(
                    "ExecutionRejected",
                    "the solver no longer wants to execute the settlement",
                ),
                StatusCode::INTERNAL_SERVER_ERROR,
            ),
            Self::Other(err) => err.into_warp_reply(),
        }
    }
}
