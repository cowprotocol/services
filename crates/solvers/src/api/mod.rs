//! Serve a solver engine API.

use {
    crate::domain::solver::Solver,
    std::{future::Future, net::SocketAddr, sync::Arc},
    tokio::sync::oneshot,
    utoipa::OpenApi,
    utoipauto::utoipauto,
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
            .route("/notify", axum::routing::post(routes::notify))
            .layer(
                tower::ServiceBuilder::new().layer(tower_http::trace::TraceLayer::new_for_http()),
            )
            .with_state(Arc::new(self.solver))
            // axum's default body limit needs to be disabled to not have the default limit on top of our custom limit
            .layer(axum::extract::DefaultBodyLimit::disable());

        let make_svc = observe::make_service_with_task_local_storage!(app);

        let server = axum::Server::bind(&self.addr).serve(make_svc);
        if let Some(bind) = bind {
            let _ = bind.send(server.local_addr());
        }

        server.with_graceful_shutdown(shutdown).await
    }
}

pub fn generate_openapi_yaml() -> Result<String, serde_yaml::Error> {
    #[utoipauto(
        paths = "./crates/solvers/src/api/routes",
        "./crates/solvers-dto/src/ from solvers_dto"
    )]
    #[derive(OpenApi)]
    #[openapi(info(
        description = "The API implemented by solver engines interacting with the reference \
                       driver implementation.",
        title = "Solver Engine API",
        version = "0.1.0",
    ))]
    pub struct ApiDoc;

    ApiDoc::openapi().to_yaml()
}
