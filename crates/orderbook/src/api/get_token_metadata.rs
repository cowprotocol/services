use {
    crate::database::Postgres,
    anyhow::Context,
    hyper::StatusCode,
    primitive_types::H160,
    std::convert::Infallible,
    warp::{reply, Filter, Rejection},
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
            Result::<_, Infallible>::Ok(
                match db
                    .token_metadata(&token)
                    .await
                    .context("get_token_metadata error")
                {
                    Ok(metadata) => reply::with_status(reply::json(&metadata), StatusCode::OK),
                    Err(err) => {
                        tracing::error!(?err, ?token, "Failed to fetch token's first trade block");
                        crate::api::internal_error_reply()
                    }
                },
            )
        }
    })
}
