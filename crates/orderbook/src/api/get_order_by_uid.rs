use {
    crate::api::AppState,
    anyhow::Result,
    axum::{
        extract::{Path, State},
        http::StatusCode,
        response::{IntoResponse, Json, Response},
    },
    model::order::{Order, OrderUid},
    std::{str::FromStr, sync::Arc},
};

pub async fn get_order_by_uid_handler(
    State(state): State<Arc<AppState>>,
    Path(uid): Path<String>,
) -> Response {
    // TODO: remove after all downstream callers have been notified of the status
    // code changes
    let Ok(uid) = OrderUid::from_str(&uid) else {
        return StatusCode::NOT_FOUND.into_response();
    };

    let result = state.orderbook.get_order(&uid).await;
    get_order_by_uid_response(result)
}

pub fn get_order_by_uid_response(result: Result<Option<Order>>) -> Response {
    let order = match result {
        Ok(order) => order,
        Err(err) => {
            tracing::error!(?err, "get_order_by_uid_response");
            return crate::api::internal_error_reply();
        }
    };
    match order {
        Some(order) => (StatusCode::OK, Json(order)).into_response(),
        None => (
            StatusCode::NOT_FOUND,
            super::error("NotFound", "Order was not found"),
        )
            .into_response(),
    }
}

#[cfg(test)]
mod tests {
    use {super::*, crate::api::response_body};

    #[tokio::test]
    async fn get_order_by_uid_response_ok() {
        let order = Order::default();
        let response = get_order_by_uid_response(Ok(Some(order.clone())));
        assert_eq!(response.status(), StatusCode::OK);
        let body = response_body(response).await;
        let response_order: Order = serde_json::from_slice(body.as_slice()).unwrap();
        assert_eq!(response_order, order);
    }

    #[tokio::test]
    async fn get_order_by_uid_response_non_existent() {
        let response = get_order_by_uid_response(Ok(None));
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }
}
