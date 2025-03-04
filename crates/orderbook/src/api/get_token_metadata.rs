use {
    crate::database::Postgres,
    hyper::StatusCode,
    primitive_types::H160,
    std::convert::Infallible,
    warp::{Filter, Rejection, reply},
};

fn get_native_prices_request() -> impl Filter<Extract = (H160,), Error = Rejection> + Clone {
    warp::path!("v1" / "token" / H160 / "metadata").and(warp::get())
}

pub fn get_token_metadata(
    db: Postgres,
) -> impl Filter<Extract = (super::ApiReply,), Error = Rejection> + Clone {
    get_native_prices_request().and_then(move |token: H160| {
        let db = db.clone();
        async move {
            let result = db.token_metadata(&token).await;
            let response = match result {
                Ok(metadata) => reply::with_status(reply::json(&metadata), StatusCode::OK),
                Err(err) => {
                    tracing::error!(?err, ?token, "Failed to fetch token's first trade block");
                    crate::api::internal_error_reply()
                }
            };

            Result::<_, Infallible>::Ok(response)
        }
    })
}
