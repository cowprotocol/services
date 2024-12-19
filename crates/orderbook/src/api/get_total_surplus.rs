use {
    crate::database::Postgres,
    primitive_types::H160,
    serde_json::json,
    std::convert::Infallible,
    warp::{http::StatusCode, reply::with_status, Filter, Rejection},
};

pub fn get(db: Postgres) -> impl Filter<Extract = (super::ApiReply,), Error = Rejection> + Clone {
    warp::path!("v1" / "users" / H160 / "total_surplus")
        .and(warp::get())
        .and_then(move |user| {
            let db = db.clone();
            async move {
                let surplus = db.total_surplus(&user).await;
                Result::<_, Infallible>::Ok(match surplus {
                    Ok(surplus) => with_status(
                        warp::reply::json(&json!({
                            "totalSurplus": surplus.to_string()
                        })),
                        StatusCode::OK,
                    ),
                    Err(err) => {
                        tracing::error!(?err, ?user, "failed to compute total surplus");
                        crate::api::internal_error_reply()
                    }
                })
            }
        })
}
