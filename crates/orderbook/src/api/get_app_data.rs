use {
    crate::database::Postgres,
    anyhow::Result,
    model::app_id::AppDataHash,
    reqwest::StatusCode,
    std::convert::Infallible,
    warp::{
        reply::{with_header, with_status},
        Filter,
        Rejection,
        Reply,
    },
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
                    let response = with_status(response, StatusCode::OK);
                    let response = with_header(response, "Content-Type", "application/json");
                    Box::new(response) as Box<dyn Reply>
                }
                Ok(None) => Box::new(with_status(
                    "full app data not found",
                    StatusCode::NOT_FOUND,
                )),
                Err(err) => {
                    tracing::error!(?err, "get_app_data_by_hash");
                    Box::new(shared::api::internal_error_reply())
                }
            })
        }
    })
}
