use crate::api::extract_payload;
use crate::database::app_data::AppDataStoring;
use crate::database::app_data::InsertionError;
use anyhow::Result;
use model::app_data::AppData;
use primitive_types::H256;
use std::{convert::Infallible, sync::Arc};
use warp::{hyper::StatusCode, Filter, Rejection, Reply};

pub fn create_app_data_request() -> impl Filter<Extract = (AppData,), Error = Rejection> + Clone {
    warp::path!("app_data")
        .and(warp::post())
        .and(extract_payload())
}

pub fn create_app_data_response(result: Result<H256, InsertionError>) -> impl Reply {
    let (body, status_code) = match result {
        Ok(hash) => (warp::reply::json(&hash), StatusCode::CREATED),
        Err(InsertionError::DuplicatedRecord(hash)) => (warp::reply::json(&hash), StatusCode::OK),
        Err(InsertionError::AnyhowError(err)) => (
            super::error("Unknown error", format!("Error is {:?}", err)),
            StatusCode::BAD_REQUEST,
        ),
        Err(InsertionError::DbError(err)) => (
            super::error("DB error", format!("Error from DB is {:?}", err)),
            StatusCode::BAD_REQUEST,
        ),
    };
    warp::reply::with_status(body, status_code)
}

pub fn create_app_data(
    database: Arc<dyn AppDataStoring>,
) -> impl Filter<Extract = (impl Reply,), Error = Rejection> + Clone {
    create_app_data_request().and_then(move |app_data| {
        let database = database.clone();
        async move {
            let result = database.insert_app_data(&app_data).await;
            if let Err(err) = &result {
                match err {
                    InsertionError::DuplicatedRecord(_hash) => (),
                    _ => tracing::error!(?err, ?app_data, "add_app_data error"),
                };
            }
            Result::<_, Infallible>::Ok(create_app_data_response(result))
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::response_body;
    use model::app_data::{AppData, MetaData};
    use serde_json::json;
    use warp::test::request;

    #[tokio::test]
    async fn create_app_data_request_ok() {
        let filter = create_app_data_request();
        let app_data_payload = json!(
        {
            "version": "1.0.0",
            "appCode": "CowSwap",
            "metaData": {
              "referrer": "0x424a46612794dbb8000194937834250dc723ffa5",
            }
        }
        );
        let request = request()
            .path("/app_data")
            .method("POST")
            .header("content-type", "application/json")
            .json(&app_data_payload);
        let result = request.filter(&filter).await.unwrap();
        let app_data = AppData {
            version: String::from("1.0.0"),
            app_code: String::from("CowSwap"),
            meta_data: MetaData {
                referrer: "0x424a46612794dbb8000194937834250dc723ffa5"
                    .parse()
                    .unwrap(),
            },
        };
        assert_eq!(result, app_data);
    }

    #[tokio::test]
    async fn create_order_response_created() {
        let hash = H256::from([1u8; 32]);
        let response = create_app_data_response(Ok(hash)).into_response();
        assert_eq!(response.status(), StatusCode::CREATED);
        let body = response_body(response).await;
        let body: serde_json::Value = serde_json::from_slice(body.as_slice()).unwrap();
        let expected = json!("0x0101010101010101010101010101010101010101010101010101010101010101");
        assert_eq!(body, expected);
    }

    #[tokio::test]
    async fn create_app_data_response_duplicate() {
        let response =
            create_app_data_response(Err(InsertionError::DuplicatedRecord(H256::zero())))
                .into_response();
        assert_eq!(response.status(), StatusCode::OK);
        let body = response_body(response).await;
        let body: serde_json::Value = serde_json::from_slice(body.as_slice()).unwrap();
        let expected_msg =
            json!("0x0000000000000000000000000000000000000000000000000000000000000000");
        assert_eq!(body, expected_msg);
    }
}
