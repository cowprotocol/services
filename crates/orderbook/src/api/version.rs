use {
    reqwest::StatusCode,
    serde_json::json,
    shared::api::ApiReply,
    std::convert::Infallible,
    warp::{reply::with_status, Filter, Rejection},
};

pub fn version() -> impl Filter<Extract = (ApiReply,), Error = Rejection> + Clone {
    warp::path!("v1" / "version")
        .and(warp::get())
        .and_then(|| async {
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
