use {
    crate::api::AppState,
    anyhow::Result,
    axum::{
        body,
        extract::State,
        http::StatusCode,
        response::{IntoResponse, Response},
    },
    futures::{Stream, StreamExt, stream::BoxStream},
    model::order::{ORDER_UID_LIMIT, Order, OrderUid},
    serde::Serialize,
    std::sync::Arc,
};

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
enum OrderResultEntry {
    Order(Order),
    Error(OrderError),
}

#[derive(Serialize)]
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
        return StatusCode::BAD_REQUEST.into_response();
    }

    get_orders_by_uid_response(state.orderbook.get_orders(&orders).await)
}

fn get_orders_by_uid_response(
    result: Result<BoxStream<'static, (OrderUid, Result<Order>)>>,
) -> Response {
    match result {
        Ok(stream) => {
            let entries = stream.map(|(uid, result)| match result {
                Ok(order) => OrderResultEntry::Order(order),
                Err(err) => {
                    tracing::warn!(?uid, ?err, "failed to convert order");
                    OrderResultEntry::Error(OrderError {
                        uid,
                        description: err.to_string(),
                    })
                }
            });
            streaming_response(entries)
        }
        Err(err) => {
            tracing::error!(?err, "get_orders_by_uid_response");
            crate::api::internal_error_reply()
        }
    }
}

fn streaming_response(entries: impl Stream<Item = OrderResultEntry> + Send + 'static) -> Response {
    let jsonl_stream = entries
        .filter_map(async move |entry| serde_json::to_string(&entry).ok())
        .map(|line| Ok::<_, std::convert::Infallible>(format!("{line}\n").into_bytes()));
    (
        [("content-type", "application/x-ndjson")],
        body::Body::from_stream(jsonl_stream),
    )
        .into_response()
}

#[cfg(test)]
mod tests {
    use {super::*, crate::api::response_body, futures::stream};

    #[tokio::test]
    async fn get_orders_by_uid_ok() {
        let order = Order::default();
        let uid = order.metadata.uid;
        let stream: BoxStream<'static, (OrderUid, Result<Order>)> =
            stream::iter(vec![(uid, Ok(order.clone()))]).boxed();
        let response = get_orders_by_uid_response(Ok(stream));
        assert_eq!(response.status(), StatusCode::OK);
        let body = response_body(response).await;
        let entries: Vec<serde_json::Value> = body
            .split(|&b| b == b'\n')
            .filter(|line| !line.is_empty())
            .map(|line| serde_json::from_slice(line).unwrap())
            .collect();
        assert_eq!(entries.len(), 1);
        assert!(entries[0].get("order").is_some());
        let response_order: Order = serde_json::from_value(entries[0]["order"].clone()).unwrap();
        assert_eq!(response_order, order);
    }

    #[tokio::test]
    async fn get_orders_by_uid_conversion_error() {
        let uid = OrderUid([1u8; 56]);
        let stream: BoxStream<'static, (OrderUid, Result<Order>)> =
            stream::iter(vec![(uid, Err(anyhow::anyhow!("bad data")))]).boxed();
        let response = get_orders_by_uid_response(Ok(stream));
        assert_eq!(response.status(), StatusCode::OK);
        let body = response_body(response).await;
        let entries: Vec<serde_json::Value> = body
            .split(|&b| b == b'\n')
            .filter(|line| !line.is_empty())
            .map(|line| serde_json::from_slice(line).unwrap())
            .collect();
        assert_eq!(entries.len(), 1);
        let error = entries[0].get("error").expect("should have error key");
        assert!(error.get("orderUid").is_some());
        assert!(error.get("description").is_some());
    }

    #[tokio::test]
    async fn get_orders_by_uid_err() {
        let response = get_orders_by_uid_response(Err(anyhow::anyhow!("error")));
        assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
    }
}
