//! Serve a solver engine API.

use {
    crate::domain::solver::Solver,
    std::{future::Future, net::SocketAddr, sync::Arc},
    tokio::{net::TcpListener, sync::oneshot},
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
            .layer(tower::ServiceBuilder::new().layer(
                tower_http::limit::RequestBodyLimitLayer::new(REQUEST_BODY_LIMIT),
            ))
            .route("/metrics", axum::routing::get(routes::metrics))
            .route("/healthz", axum::routing::get(routes::healthz))
            .route("/solve", axum::routing::post(routes::solve))
            .layer(
                tower::ServiceBuilder::new().layer(tower_http::trace::TraceLayer::new_for_http()),
            )
            .with_state(Arc::new(self.solver))
            // axum's default body limit needs to be disabled to not have the default limit on top of our custom limit
            .layer(axum::extract::DefaultBodyLimit::disable());

        let make_svc = observe::make_service_with_task_local_storage!(app);

        let listener = TcpListener::bind(&self.addr).await.unwrap();
        let addr = listener.local_addr().unwrap();
        let server = axum::serve(listener, make_svc);
        if let Some(bind) = bind {
            let _ = bind.send(addr);
        }

        server.with_graceful_shutdown(shutdown).await
    }
}
