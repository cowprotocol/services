use {
    crate::{solver::Solver, Ethereum, Simulator},
    futures::Future,
    std::{net::SocketAddr, sync::Arc},
};

pub mod execute;
pub mod info;
pub mod solve;

const REQUEST_BODY_LIMIT: usize = 10 * 1024 * 1024;

pub struct Api {
    pub solvers: Vec<Solver>,
    pub simulator: Simulator,
    pub eth: Ethereum,
    pub addr: SocketAddr,
}

impl Api {
    pub async fn serve(
        self,
        shutdown: impl Future<Output = ()> + Send + 'static,
    ) -> Result<(), hyper::Error> {
        // Add middleware.
        let mut app = axum::Router::new().layer(
            tower::ServiceBuilder::new()
                .layer(tower_http::limit::RequestBodyLimitLayer::new(
                    REQUEST_BODY_LIMIT,
                ))
                .layer(tower_http::trace::TraceLayer::new_for_http()),
        );

        // Multiplex each solver as part of the API.
        let shared = Arc::new(SharedState {
            simulator: self.simulator,
            eth: self.eth,
        });
        for solver in self.solvers {
            let name = solver.name().clone();
            let router = axum::Router::new();
            let router = solve::route(router);
            let router = info::route(router);
            let router = router.with_state(State {
                solver,
                shared: Arc::clone(&shared),
            });
            app = app.nest(&name.0, router);
        }

        // Start the server.
        axum::Server::bind(&self.addr)
            .serve(app.into_make_service())
            .with_graceful_shutdown(shutdown)
            .await
    }
}

#[derive(Debug, Clone)]
struct State {
    solver: Solver,
    shared: Arc<SharedState>,
}

impl State {
    fn solver(&self) -> Solver {
        self.solver.clone()
    }

    fn simulator(&self) -> &Simulator {
        &self.shared.simulator
    }

    fn ethereum(&self) -> &Ethereum {
        &self.shared.eth
    }
}

/// State which is shared among all multiplexed solvers.
#[derive(Debug)]
struct SharedState {
    simulator: Simulator,
    eth: Ethereum,
}
