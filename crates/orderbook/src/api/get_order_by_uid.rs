use {
    super::with_status,
    anyhow::Result,
    axum::{http::StatusCode, routing::MethodRouter},
    model::order::{Order, OrderUid},
    shared::api::{error, ApiReply},
};

pub fn route() -> (&'static str, MethodRouter<super::State>) {
    (ENDPOINT, axum::routing::get(handler))
}

const ENDPOINT: &str = "/api/v1/orders/:uid";
async fn handler(
    state: axum::extract::State<super::State>,
    uid: axum::extract::Path<OrderUid>,
) -> ApiReply {
    let result = state.orderbook.get_order(&uid.0).await;
    get_order_by_uid_response(result)
}

fn get_order_by_uid_response(result: Result<Option<Order>>) -> ApiReply {
    let order = match result {
        Ok(order) => order,
        Err(err) => {
            tracing::error!(?err, "get_order_by_uid_response");
            return shared::api::internal_error_reply();
        }
    };
    match order {
        Some(order) => with_status(serde_json::to_value(&order).unwrap(), StatusCode::OK),
        None => with_status(
            error("NotFound", "Order was not found"),
            StatusCode::NOT_FOUND,
        ),
    }
}

// #[cfg(test)]
// mod tests {
//     use {
//         super::*,
//         shared::api::response_body,
//         warp::{test::request, Reply},
//     };

//     #[tokio::test]
//     async fn get_order_by_uid_request_ok() {
//         let uid = OrderUid::default();
//         let request =
// request().path(&format!("/v1/orders/{uid}")).method("GET");         let
// filter = get_order_by_uid_request();         let result =
// request.filter(&filter).await.unwrap();         assert_eq!(result, uid);
//     }

//     #[tokio::test]
//     async fn get_order_by_uid_response_ok() {
//         let order = Order::default();
//         let response =
// get_order_by_uid_response(Ok(Some(order.clone()))).into_response();
//         assert_eq!(response.status(), StatusCode::OK);
//         let body = response_body(response).await;
//         let response_order: Order =
// serde_json::from_slice(body.as_slice()).unwrap();         assert_eq!
// (response_order, order);     }

//     #[tokio::test]
//     async fn get_order_by_uid_response_non_existent() {
//         let response = get_order_by_uid_response(Ok(None)).into_response();
//         assert_eq!(response.status(), StatusCode::NOT_FOUND);
//     }
// }
