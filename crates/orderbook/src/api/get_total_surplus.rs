use {
    primitive_types::H160,
    serde_json::json,
    std::convert::Infallible,
    warp::{http::StatusCode, reply::with_status, Filter, Rejection},
};

pub fn get() -> impl Filter<Extract = (super::ApiReply,), Error = Rejection> + Clone {
    warp::path!("v1" / "users" / H160 / "total_surplus")
        .and(warp::get())
        .and_then(move |_| async {
            Result::<_, Infallible>::Ok(with_status(
                warp::reply::json(&json!({
                    "totalSurplus": 42_133_700_000_000_000_000_u128.to_string(),
                })),
                StatusCode::OK,
            ))
        })
}
