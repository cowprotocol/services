use {
    crate::infra::delta_sync,
    axum::{
        Json,
        http::StatusCode,
        response::{IntoResponse, Response},
        routing::get,
    },
};

pub(in crate::infra::api) fn healthz(app: axum::Router<()>) -> axum::Router<()> {
    app.route("/healthz", get(route))
        .route("/health/delta-replica", get(delta_replica))
}

async fn route() -> Response {
    StatusCode::OK.into_response()
}

async fn delta_replica() -> Response {
    match delta_sync::replica_health().await {
        Some(health) => Json(health).into_response(),
        None => (StatusCode::SERVICE_UNAVAILABLE, "delta replica disabled").into_response(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn delta_replica_returns_service_unavailable_when_disabled() {
        let response = delta_replica().await;
        assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
    }
}
