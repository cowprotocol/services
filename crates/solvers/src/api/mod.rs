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
    ) -> Result<(), std::io::Error> {
        let app = axum::Router::new()
            .route("/metrics", axum::routing::get(routes::metrics))
            .route("/healthz", axum::routing::get(routes::healthz))
            .route("/solve", axum::routing::post(routes::solve))
            .with_state(Arc::new(self.solver))
            // Layers are placed as in a stack, so this is the inner most layer
            // axum's default body limit needs to be disabled to not have the default limit on top of our custom limit
            .layer(axum::extract::DefaultBodyLimit::disable())
            .layer(tower_http::limit::RequestBodyLimitLayer::new(REQUEST_BODY_LIMIT))
                        .layer(axum::middleware::from_fn(
                |req, next: axum::middleware::Next| async move {
                    let req = record_trace_id(req);
                    next.run(req).await
                },
            ))
            .layer(tower_http::trace::TraceLayer::new_for_http().make_span_with(make_span));

        let listener = tokio::net::TcpListener::bind(&self.addr).await?;
        let local_addr = listener.local_addr()?;
        if let Some(bind) = bind {
            let _ = bind.send(local_addr);
        }

        axum::serve(listener, app)
            .with_graceful_shutdown(shutdown)
            .await
    }
}
