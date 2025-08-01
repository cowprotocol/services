//! Serve a solver engine API.

use {
    crate::domain::solver::Solver,
    observe::distributed_tracing::tracing_axum::{make_span, record_trace_id},
    std::{future::Future, net::SocketAddr, sync::Arc},
    tokio::sync::oneshot,
};

mod routes;

const REQUEST_BODY_LIMIT: usize = 10 * 1024 * 1024;

pub struct Api {
    pub addr: SocketAddr,
    pub solver: Solver,
}

impl Api {
    pub async fn serve(
        self,
        bind: Option<oneshot::Sender<SocketAddr>>,
        shutdown: impl Future<Output = ()> + Send + 'static,
    ) -> Result<(), hyper::Error> {
        let app = axum::Router::new()
            .layer(tower::ServiceBuilder::new().layer(
                tower_http::limit::RequestBodyLimitLayer::new(REQUEST_BODY_LIMIT),
            ))
            .route("/metrics", axum::routing::get(routes::metrics))
            .route("/healthz", axum::routing::get(routes::healthz))
            .route("/solve", axum::routing::post(routes::solve))
            .layer(
                tower::ServiceBuilder::new()
                    .layer(tower_http::trace::TraceLayer::new_for_http().make_span_with(make_span))
                    .map_request(record_trace_id),
            )
            .with_state(Arc::new(self.solver))
            // axum's default body limit needs to be disabled to not have the default limit on top of our custom limit
            .layer(axum::extract::DefaultBodyLimit::disable());

        let server = axum::Server::bind(&self.addr).serve(app.into_make_service());
        if let Some(bind) = bind {
            let _ = bind.send(server.local_addr());
        }

        server.with_graceful_shutdown(shutdown).await
    }
}
