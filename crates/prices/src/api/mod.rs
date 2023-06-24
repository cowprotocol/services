use {
    crate::core,
    futures::Future,
    std::{net::SocketAddr, sync::Arc},
    tokio::sync::oneshot,
};

mod routes;

const REQUEST_BODY_LIMIT: usize = 10 * 1024 * 1024;

pub struct Api {
    pub estimators: Vec<Box<dyn core::Estimator>>,
    pub addr: SocketAddr,
    /// If this channel is specified, the bound address will be sent to it. This
    /// allows the driver to bind to 0.0.0.0:0 during testing.
    pub addr_sender: Option<oneshot::Sender<SocketAddr>>,
}

impl Api {
    pub async fn serve(
        self,
        shutdown: impl Future<Output = ()> + Send + 'static,
    ) -> Result<(), hyper::Error> {
        // Add middleware.
        let router = axum::Router::new().layer(
            tower::ServiceBuilder::new()
                .layer(tower_http::limit::RequestBodyLimitLayer::new(
                    REQUEST_BODY_LIMIT,
                ))
                .layer(tower_http::trace::TraceLayer::new_for_http()),
        );

        // Add routes.
        let router = routes::info(router);
        let router = routes::estimate(router);

        // Add state.
        let router = router.with_state(State(Arc::new(Inner {
            estimators: self.estimators,
        })));

        // Start the server.
        let server = axum::Server::bind(&self.addr).serve(router.into_make_service());
        if let Some(addr_sender) = self.addr_sender {
            addr_sender.send(server.local_addr()).unwrap();
        }
        server.with_graceful_shutdown(shutdown).await
    }
}

#[derive(Debug, Clone)]
struct State(Arc<Inner>);

impl State {
    fn estimators(&self) -> &[Box<dyn core::Estimator>] {
        &self.0.estimators
    }
}

#[derive(Debug)]
struct Inner {
    estimators: Vec<Box<dyn core::Estimator>>,
}
