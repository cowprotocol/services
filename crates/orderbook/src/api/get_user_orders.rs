use {
    crate::api::with_status,
    axum::{http::StatusCode, routing::MethodRouter},
    primitive_types::H160,
    serde::Deserialize,
    shared::api::{error, internal_error_reply, ApiReply},
};

pub fn route() -> (&'static str, MethodRouter<super::State>) {
    (ENDPOINT, axum::routing::get(handler))
}

const ENDPOINT: &str = "/api/v1/account/:owner/orders";
async fn handler(
    state: axum::extract::State<super::State>,
    owner: axum::extract::Path<H160>,
    query: Option<axum::extract::Json<Query>>,
) -> ApiReply {
    const DEFAULT_OFFSET: u64 = 0;
    const DEFAULT_LIMIT: u64 = 10;
    const MIN_LIMIT: u64 = 1;
    const MAX_LIMIT: u64 = 1000;

    let offset = query
        .as_ref()
        .map(|q| q.offset)
        .flatten()
        .unwrap_or(DEFAULT_OFFSET);
    let limit = query
        .as_ref()
        .map(|q| q.limit)
        .flatten()
        .unwrap_or(DEFAULT_LIMIT);

    if !(MIN_LIMIT..=MAX_LIMIT).contains(&limit) {
        return with_status(
            error(
                "LIMIT_OUT_OF_BOUNDS",
                format!("The pagination limit is [{MIN_LIMIT},{MAX_LIMIT}]."),
            ),
            StatusCode::BAD_REQUEST,
        );
    }
    let result = state.orderbook.get_user_orders(&owner, offset, limit).await;
    match result {
        Ok(reply) => with_status(serde_json::to_value(&reply).unwrap(), StatusCode::OK),
        Err(err) => {
            tracing::error!(?err, "get_user_orders");
            internal_error_reply()
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize)]
struct Query {
    offset: Option<u64>,
    limit: Option<u64>,
}

// #[cfg(test)]
// mod tests {
//     use {super::*, shared::addr};

//     #[tokio::test]
//     async fn request_() {
//         let path =
// "/v1/account/0x0000000000000000000000000000000000000001/orders";         let
// result = warp::test::request()             .path(path)
//             .method("GET")
//             .filter(&request())
//             .await
//             .unwrap();
//         assert_eq!(result.0,
// addr!("0000000000000000000000000000000000000001"));         assert_eq!
// (result.1.offset, None);         assert_eq!(result.1.limit, None);

//         let path =
// "/v1/account/0x0000000000000000000000000000000000000001/orders?offset=1&
// limit=2";         let result = warp::test::request()
//             .path(path)
//             .method("GET")
//             .filter(&request())
//             .await
//             .unwrap();
//         assert_eq!(result.1.offset, Some(1));
//         assert_eq!(result.1.limit, Some(2));
//     }
// }
