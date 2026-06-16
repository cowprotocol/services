use {
    crate::domain::proposal::ProposalStore,
    alloy_sol_types::Eip712Domain,
    observe::tracing::distributed::axum::{make_span, record_trace_id},
    std::{future::Future, net::SocketAddr, sync::Arc},
    tokio::sync::oneshot,
};

mod routes;

const REQUEST_BODY_LIMIT: usize = 10 * 1024 * 1024;

pub struct AppState {
    pub store: ProposalStore,
    pub domain: Eip712Domain,
}

pub struct Api {
    pub addr: SocketAddr,
    pub state: AppState,
}

impl Api {
    pub async fn serve(
        self,
        bind: Option<oneshot::Sender<SocketAddr>>,
        shutdown: impl Future<Output = ()> + Send + 'static,
    ) -> Result<(), std::io::Error> {
        let app = axum::Router::new()
            .layer(tower::ServiceBuilder::new().layer(
                tower_http::limit::RequestBodyLimitLayer::new(REQUEST_BODY_LIMIT),
            ))
            .route("/metrics", axum::routing::get(routes::metrics))
            .route("/healthz", axum::routing::get(routes::healthz))
            .route("/solve", axum::routing::post(routes::solve))
            .route("/proposals", axum::routing::post(routes::submit_proposal))
            .route(
                "/proposals/{order_uid}",
                axum::routing::get(routes::get_proposals),
            )
            .route(
                "/proposals/{id}/cancel",
                axum::routing::delete(routes::cancel_proposal),
            )
            .layer(
                tower::ServiceBuilder::new()
                    .layer(tower_http::trace::TraceLayer::new_for_http().make_span_with(make_span))
                    .map_request(record_trace_id),
            )
            .with_state(Arc::new(self.state))
            .layer(axum::extract::DefaultBodyLimit::disable());

        let listener = tokio::net::TcpListener::bind(self.addr).await?;
        if let Some(bind) = bind {
            let _ = bind.send(listener.local_addr()?);
        }

        axum::serve(listener, app)
            .with_graceful_shutdown(shutdown)
            .await
    }
}
