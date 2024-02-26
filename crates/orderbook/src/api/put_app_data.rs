use {
    crate::app_data,
    anyhow::Result,
    model::app_data::{AppDataDocument, AppDataHash},
    reqwest::StatusCode,
    shared::api::{internal_error_reply, IntoWarpReply},
    std::{convert::Infallible, sync::Arc},
    warp::{body, reply, Filter, Rejection},
};

fn request(
    max_size: usize,
) -> impl Filter<Extract = (Option<AppDataHash>, AppDataDocument), Error = Rejection> + Clone {
    let opt = warp::path::param::<AppDataHash>()
        .map(Some)
        .or_else(|_| async { Ok::<(Option<AppDataHash>,), std::convert::Infallible>((None,)) });
    warp::path!("v1" / "app_data" / ..)
        .and(opt)
        .and(warp::put())
        .and(body::content_length_limit(max_size as _))
        .and(body::json())
}

fn response(
    result: Result<(app_data::Registered, AppDataHash), app_data::RegisterError>,
) -> super::ApiReply {
    match result {
        Ok((registered, hash)) => {
            let status = match registered {
                app_data::Registered::New => StatusCode::CREATED,
                app_data::Registered::AlreadyExisted => StatusCode::OK,
            };
            reply::with_status(reply::json(&hash), status)
        }
        Err(err) => err.into_warp_reply(),
    }
}

pub fn filter(
    registry: Arc<app_data::Registry>,
) -> impl Filter<Extract = (super::ApiReply,), Error = Rejection> + Clone {
    request(registry.size_limit()).and_then(move |hash, document: AppDataDocument| {
        let registry = registry.clone();
        async move {
            let result = registry
                .register(hash, document.full_app_data.as_bytes())
                .await;
            Result::<_, Infallible>::Ok(response(result))
        }
    })
}

impl IntoWarpReply for app_data::RegisterError {
    fn into_warp_reply(self) -> super::ApiReply {
        match self {
            Self::Invalid(err) => reply::with_status(
                super::error("AppDataInvalid", err.to_string()),
                StatusCode::BAD_REQUEST,
            ),
            err @ Self::HashMismatch { .. } => reply::with_status(
                super::error("AppDataHashMismatch", err.to_string()),
                StatusCode::BAD_REQUEST,
            ),
            err @ Self::DataMismatch { .. } => reply::with_status(
                super::error("AppDataMismatch", err.to_string()),
                StatusCode::BAD_REQUEST,
            ),
            Self::Other(err) => {
                tracing::error!(?err, "app_data::SaveError::Other");
                internal_error_reply()
            }
        }
    }
}
