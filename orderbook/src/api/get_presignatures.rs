use crate::api::convert_get_presignatures_error_to_reply;
use crate::database::presignatures::PreSignatureFilter;
use crate::database::presignatures::PreSignatureRetrieving;
use anyhow::Result;
use futures::TryStreamExt;
use model::order::OrderUid;
use model::presignature::PreSignature;
use serde::Deserialize;
use shared::H160Wrapper;
use std::convert::Infallible;
use std::sync::Arc;
use warp::reply::{Json, WithStatus};
use warp::{hyper::StatusCode, Filter, Rejection, Reply};

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Query {
    pub order_uid: Option<OrderUid>,
    pub owner: Option<H160Wrapper>,
}

#[derive(Debug, Eq, PartialEq)]
enum PreSignatureFilterError {
    InvalidFilter(String),
}

impl Query {
    fn presignature_filter(&self) -> PreSignatureFilter {
        let to_h160 = |option: Option<&H160Wrapper>| option.map(|wrapper| wrapper.0);
        PreSignatureFilter {
            order_uid: self.order_uid,
            owner: to_h160(self.owner.as_ref()),
        }
    }

    fn validate(&self) -> Result<PreSignatureFilter, PreSignatureFilterError> {
        match (self.order_uid.as_ref(), self.owner.as_ref()) {
            (None, None) => Err(PreSignatureFilterError::InvalidFilter(
                "Must specify at least one of owner and order_uid.".to_owned(),
            )),
            _ => Ok(self.presignature_filter()),
        }
    }
}

fn get_presignatures_request(
) -> impl Filter<Extract = (Result<PreSignatureFilter, PreSignatureFilterError>,), Error = Rejection>
       + Clone {
    warp::path!("presignatures")
        .and(warp::get())
        .and(warp::query::<Query>())
        .map(|query: Query| query.validate())
}

fn get_presignatures_response(result: Result<Vec<PreSignature>>) -> WithStatus<Json> {
    match result {
        Ok(presignatures) => {
            warp::reply::with_status(warp::reply::json(&presignatures), StatusCode::OK)
        }
        Err(err) => convert_get_presignatures_error_to_reply(err),
    }
}

pub fn get_presignatures(
    db: Arc<dyn PreSignatureRetrieving>,
) -> impl Filter<Extract = (impl Reply,), Error = Rejection> + Clone {
    get_presignatures_request().and_then(move |request_result| {
        let database = db.clone();
        async move {
            match request_result {
                Ok(presignatures) => {
                    let result = database
                        .presignatures(&presignatures)
                        .try_collect::<Vec<_>>()
                        .await;
                    Result::<_, Infallible>::Ok(get_presignatures_response(result))
                }
                Err(PreSignatureFilterError::InvalidFilter(msg)) => {
                    let err = super::error("InvalidPreSignatureFilter", msg);
                    Ok(warp::reply::with_status(err, StatusCode::BAD_REQUEST))
                }
            }
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::response_body;
    use hex_literal::hex;
    use primitive_types::H160;
    use warp::test::{request, RequestBuilder};

    #[tokio::test]
    async fn get_presignatures_request_ok() {
        let presignatures = |request: RequestBuilder| async move {
            let filter = get_presignatures_request();
            request.method("GET").filter(&filter).await
        };

        let owner = H160::from_slice(&hex!("0000000000000000000000000000000000000001"));
        let owner_path = format!("/presignatures?owner=0x{:x}", owner);
        let result = presignatures(request().path(owner_path.as_str()))
            .await
            .unwrap()
            .unwrap();
        assert_eq!(result.owner, Some(owner));
        assert_eq!(result.order_uid, None);

        let uid = OrderUid([1u8; 56]);
        let order_uid_path = format!("/presignatures?orderUid={:}", uid);
        let result = presignatures(request().path(order_uid_path.as_str()))
            .await
            .unwrap()
            .unwrap();
        assert_eq!(result.owner, None);
        assert_eq!(result.order_uid, Some(uid));

        let path = format!("/presignatures?owner=0x{:x}&orderUid={:}", owner, uid);

        let result = presignatures(request().path(path.as_str()))
            .await
            .unwrap()
            .unwrap();
        assert_eq!(result.owner, Some(owner));
        assert_eq!(result.order_uid, Some(uid));
    }

    #[tokio::test]
    async fn get_presignatures_request_err() {
        let presignatures = |request: RequestBuilder| async move {
            let filter = get_presignatures_request();
            request.method("GET").filter(&filter).await
        };

        let path = "/presignatures";
        let result = presignatures(request().path(path)).await.unwrap();
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn get_presignatures_response_ok() {
        let presignatures = vec![PreSignature::default()];
        let response = get_presignatures_response(Ok(presignatures.clone())).into_response();
        assert_eq!(response.status(), StatusCode::OK);
        let body = response_body(response).await;
        let response_presignatures: Vec<PreSignature> =
            serde_json::from_slice(body.as_slice()).unwrap();
        assert_eq!(response_presignatures, presignatures);
    }
}
