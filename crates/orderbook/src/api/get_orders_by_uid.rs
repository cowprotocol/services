use {
    crate::api::AppState,
    anyhow::Result,
    axum::{
        body,
        extract::State,
        http::StatusCode,
        response::{IntoResponse, Response},
    },
    futures::{
        Stream,
        StreamExt,
        stream::{self, BoxStream},
    },
    model::order::{ORDER_UID_LIMIT, Order, OrderUid},
    std::sync::Arc,
};

pub async fn get_orders_by_uid_handler(
    State(state): State<Arc<AppState>>,
    orders: Request<Vec<OrderUid>>
) -> Response {

    if orders.len() > ORDER_UID_LIMIT {
        return StatusCode::BAD_REQUEST.into_response();
    }

    get_orders_by_uid_response(state.orderbook.get_orders(&orders).await)
}

fn get_orders_by_uid_response(result: Result<BoxStream<'static, Result<Order>>>) -> Response {
    match result {
        Ok(stream) => {
            let orders = stream.filter_map(async |item| item.inspect_err(|err| tracing::warn!(?err, "failed to fetch order")).ok();
            streaming_response(orders)
        }
        Err(err) => {
            tracing::error!(?err, "get_orders_by_uid_response");
            crate::api::internal_error_reply()
        }
    }
}

fn streaming_json_array(
    elements: impl Stream<Item = String> + Send + 'static,
) -> impl Stream<Item = String> + Send + 'static {
    let mut first = true;
    stream::once(async { "[".to_string() })
        .chain(elements.map(move |element| {
            let prefix = if first { "" } else { "," };
            first = false;
            format!("{prefix}{element}")
        }))
        .chain(stream::once(async { "]".to_string() }))
}

fn streaming_response(orders: impl Stream<Item = Order> + Send + 'static) -> Response {
    let json_stream = streaming_json_array(
        orders.filter_map(async move |order| serde_json::to_string(&order).ok()),
    )
    .map(|s| Ok::<_, std::convert::Infallible>(s.into_bytes()));
    (
        [("content-type", "application/json")],
        body::Body::from_stream(json_stream),
    )
        .into_response()
}

#[cfg(test)]
mod tests {
    use {super::*, crate::api::response_body, futures::stream};

    #[tokio::test]
    async fn get_orders_by_uid_ok() {
        let order = [Order::default()];
        let stream: BoxStream<'static, Result<Order>> = stream::iter(order.clone().map(Ok)).boxed();
        let response = get_orders_by_uid_response(Ok(stream));
        assert_eq!(response.status(), StatusCode::OK);
        let body = response_body(response).await;
        let response_order: Vec<Order> = serde_json::from_slice(body.as_slice()).unwrap();
        assert!(response_order.eq(&order));
    }

    #[tokio::test]
    async fn get_orders_by_uid_err() {
        let response = get_orders_by_uid_response(Err(anyhow::anyhow!("error")));
        assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
    }
}
