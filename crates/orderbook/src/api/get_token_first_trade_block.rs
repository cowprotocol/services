use {
    crate::database::Postgres,
    anyhow::Context,
    hyper::StatusCode,
    primitive_types::H160,
    std::convert::Infallible,
    warp::{
        reply::{json, with_status},
        Filter,
        Rejection,
    },
};

fn get_native_prices_request() -> impl Filter<Extract = (H160,), Error = Rejection> + Clone {
    warp::path!("v1" / "token" / H160 / "first_trade_block").and(warp::get())
}

pub fn get_token_first_trade_block(
    db: Postgres,
) -> impl Filter<Extract = (super::ApiReply,), Error = Rejection> + Clone {
    get_native_prices_request().and_then(move |token: H160| {
        let db = db.clone();
        async move {
            Result::<_, Infallible>::Ok(
                match db
                    .token_first_trade_block(&token)
                    .await
                    .context("get_token_first_trade_block error")
                {
                    Ok(Some(block)) => with_status(json(&block), StatusCode::OK),
                    Ok(None) => with_status(
                        super::error("NotFound", "no trade for token exists"),
                        StatusCode::NOT_FOUND,
                    ),
                    Err(err) => {
                        tracing::error!(?err, ?token, "Failed to fetch token's first trade block");
                        crate::api::internal_error_reply()
                    }
                },
            )
        }
    })
}
