use {
    crate::database::Postgres,
    anyhow::Result,
    app_data::{AppDataDocument, AppDataHash},
    reqwest::StatusCode,
    std::convert::Infallible,
    warp::{reply, Filter, Rejection, Reply},
};

pub fn request() -> impl Filter<Extract = (AppDataHash,), Error = Rejection> + Clone {
    warp::path!("v1" / "app_data" / AppDataHash).and(warp::get())
}

pub fn get(
    database: Postgres,
) -> impl Filter<Extract = (Box<dyn Reply>,), Error = Rejection> + Clone {
    request().and_then(move |contract_app_data: AppDataHash| {
        let database = database.clone();
        async move {
            let result = database.get_full_app_data(&contract_app_data).await;
            Result::<_, Infallible>::Ok(match result {
                Ok(Some(response)) => {
                    let response = reply::with_status(
                        reply::json(&AppDataDocument {
                            full_app_data: response,
                        }),
                        StatusCode::OK,
                    );
                    Box::new(response) as Box<dyn Reply>
                }
                Ok(None) => Box::new(reply::with_status(
                    "full app data not found",
                    StatusCode::NOT_FOUND,
                )),
                Err(err) => {
                    tracing::error!(?err, "get_app_data_by_hash");
                    Box::new(crate::api::internal_error_reply())
                }
            })
        }
    })
}
