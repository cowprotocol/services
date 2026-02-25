use {
    crate::api::AppState,
    anyhow::Result,
    axum::{
        extract::State,
        http::StatusCode,
        response::{IntoResponse, Response},
    },
    model::order::{ORDER_UID_LIMIT, Order, OrderUid},
    serde::Serialize,
    std::sync::Arc,
};

#[expect(clippy::large_enum_variant)]
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
enum OrderResultEntry {
    Order(Order),
    Error(OrderError),
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct OrderError {
    uid: OrderUid,
    description: String,
}

pub async fn get_orders_by_uid_handler(
    State(state): State<Arc<AppState>>,
    axum::Json(orders): axum::Json<Vec<OrderUid>>,
) -> Response {
    if orders.len() > ORDER_UID_LIMIT {
        return (
            StatusCode::BAD_REQUEST,
            format!("Request exceeds maximum number of order UIDs of {ORDER_UID_LIMIT}"),
        )
            .into_response();
    }

    get_orders_by_uid_response(state.orderbook.get_orders(&orders).await)
}

fn get_orders_by_uid_response(result: Result<Vec<(OrderUid, Result<Order>)>>) -> Response {
    match result {
        Ok(orders) => axum::Json(
            orders
                .into_iter()
                .map(|(uid, order)| match order {
                    Ok(order) => OrderResultEntry::Order(order),
                    Err(err) => {
                        tracing::warn!(?err, "Error converting into model order");
                        OrderResultEntry::Error(OrderError {
                            uid,
                            description: "Internal server error encountered when retrieving the \
                                          order"
                                .to_string(),
                        })
                    }
                })
                .collect::<Vec<OrderResultEntry>>(),
        )
        .into_response(),
        Err(err) => {
            tracing::error!(?err, "get_orders_by_uid_response");
            crate::api::internal_error_reply()
        }
    }
}

#[cfg(test)]
mod tests {
    use {super::*, crate::api::response_body};

    #[tokio::test]
    async fn get_orders_by_uid_ok() {
        let order = Order::default();
        let uid = order.metadata.uid;
        let result = vec![(uid, Ok(order.clone()))];
        let response = get_orders_by_uid_response(Ok(result));
        assert_eq!(response.status(), StatusCode::OK);

        let body = response_body(response).await;
        let entries: Vec<serde_json::Value> = serde_json::from_slice(&body).unwrap();
        assert_eq!(entries.len(), 1);

        let order_entry: Order =
            serde_json::from_value(entries[0].get("order").expect("key order exists").clone())
                .expect("value is a correct Order");
        assert_eq!(order_entry, order);
    }

    #[tokio::test]
    async fn get_orders_by_uid_conversion_error() {
        let uid = OrderUid([1u8; 56]);
        let result = vec![(uid, Err(anyhow::anyhow!("bad data")))];
        let response = get_orders_by_uid_response(Ok(result));
        assert_eq!(response.status(), StatusCode::OK);

        let body = response_body(response).await;
        let entries: Vec<serde_json::Value> = serde_json::from_slice(&body).unwrap();
        assert_eq!(entries.len(), 1);

        let error = entries[0].get("error").expect("error key exists");
        let error_uid: OrderUid = error.get("uid").unwrap().as_str().unwrap().parse().unwrap();
        assert_eq!(error_uid, uid);
        assert_eq!(
            error
                .get("description")
                .expect("key description exists")
                .as_str()
                .expect("description is a string"),
            "Error converting into model order"
        );
    }

    #[tokio::test]
    async fn get_orders_by_uid_err() {
        let response = get_orders_by_uid_response(Err(anyhow::anyhow!("error")));
        assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
    }
}
