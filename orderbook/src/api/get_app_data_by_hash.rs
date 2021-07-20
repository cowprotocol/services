use crate::api::convert_get_app_data_error_to_reply;
use crate::database::app_data::AppDataFilter;
use crate::database::app_data::AppDataStoring;
use anyhow::Result;
use futures::TryStreamExt;
use model::app_data::AppData;
use shared::H256Wrapper;

use std::{convert::Infallible, sync::Arc};
use warp::{hyper::StatusCode, reply, Filter, Rejection, Reply};

pub fn get_app_data_by_hash_request(
) -> impl Filter<Extract = (AppDataFilter,), Error = Rejection> + Clone {
    warp::path!("app_data" / H256Wrapper)
        .and(warp::get())
        .map(|hash: H256Wrapper| AppDataFilter {
            app_data_hash: Some(hash.0),
            referrer: None,
        })
}

pub fn get_app_data_by_hash_response(result: Result<Vec<AppData>>) -> impl Reply {
    let app_data = match result {
        Ok(app_data) => app_data,
        Err(err) => {
            return Ok(convert_get_app_data_error_to_reply(err));
        }
    };
    Ok(match app_data.first() {
        Some(app_data) => reply::with_status(reply::json(&app_data), StatusCode::OK),
        None => reply::with_status(
            super::error("NotFound", "AppDataHash was not found"),
            StatusCode::NOT_FOUND,
        ),
    })
}

pub fn get_app_data_by_hash(
    database: Arc<dyn AppDataStoring>,
) -> impl Filter<Extract = (impl Reply,), Error = Rejection> + Clone {
    get_app_data_by_hash_request().and_then(move |app_data_filter| {
        let database = database.clone();
        async move {
            let result = database
                .app_data(&app_data_filter)
                .try_collect::<Vec<_>>()
                .await;
            Result::<_, Infallible>::Ok(get_app_data_by_hash_response(result))
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::response_body;
    use warp::test::request;

    #[tokio::test]
    async fn get_app_data_by_hash_request_ok() {
        let app_data_hash =
            String::from("0x0000000000000000000000000000000000000000000000000000000000000001");
        let path_string = format!("/app_data/{}", app_data_hash);
        let request = request().path(&path_string).method("GET");
        let filter = get_app_data_by_hash_request();
        let result = request.filter(&filter).await.unwrap();
        let expected: primitive_types::H256 = app_data_hash.parse().unwrap();
        assert_eq!(result.app_data_hash, Some(expected));
    }

    #[tokio::test]
    async fn get_app_data_by_hash_response_ok() {
        let app_data = AppData {
            version: String::from("1.0.0"),
            app_code: String::from("CowSwap"),
            meta_data: model::app_data::MetaData {
                referrer: "0x424a46612794dbb8000194937834250dc723ffa5"
                    .parse()
                    .unwrap(),
            },
        };
        let response = get_app_data_by_hash_response(Ok(vec![app_data.clone()])).into_response();
        assert_eq!(response.status(), StatusCode::OK);
        let body = response_body(response).await;
        let response: AppData = serde_json::from_slice(body.as_slice()).unwrap();
        assert_eq!(response, app_data);
    }

    #[tokio::test]
    async fn get_app_data_by_hash_response_non_existent() {
        let response = get_app_data_by_hash_response(Ok(Vec::new())).into_response();
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }
}
