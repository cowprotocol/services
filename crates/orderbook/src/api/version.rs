use reqwest::StatusCode;
use serde_json::json;
use shared::api::ApiReply;
use std::convert::Infallible;
use warp::{reply::with_status, Filter, Rejection};

pub fn version() -> impl Filter<Extract = (ApiReply,), Error = Rejection> + Clone {
    warp::path!("version").and(warp::get()).and_then(|| async {
        Result::<_, Infallible>::Ok(with_status(
            warp::reply::json(&json!({
                "version": env!("VERGEN_GIT_SEMVER_LIGHTWEIGHT"),
                "commit": env!("VERGEN_GIT_SHA"),
                "branch": env!("VERGEN_GIT_BRANCH"),
            })),
            StatusCode::OK,
        ))
    })
}
