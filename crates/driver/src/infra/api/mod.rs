use {
    crate::{domain::competition, infra, solver::Solver, Ethereum, Simulator},
    futures::Future,
    std::{net::SocketAddr, sync::Arc},
    tokio::sync::oneshot,
};

mod execute;
mod info;
mod quote;
mod solve;

const REQUEST_BODY_LIMIT: usize = 10 * 1024 * 1024;

pub enum Addr {
    /// Bind to a specific port and address.
    Bind(SocketAddr),
    /// Bind to 0.0.0.0 and any free port, then send the bound address down the
    /// oneshot channel if specified.
    Auto(Option<oneshot::Sender<SocketAddr>>),
}

pub struct Api {
    pub solvers: Vec<Solver>,
    pub simulator: Simulator,
    pub eth: Ethereum,
    pub now: infra::time::Now,
    pub quote_config: competition::quote::Config,
    pub addr: Addr,
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
            now: self.now,
            quote_config: self.quote_config,
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
            app = app.nest(&format!("/{name}"), router);
        }

        // Start the server.

        let server = match self.addr {
            Addr::Bind(addr) => axum::Server::bind(&addr).serve(app.into_make_service()),
            Addr::Auto(addr_sender) => {
                let server = axum::Server::bind(&"0.0.0.0:0".parse().unwrap())
                    .serve(app.into_make_service());
                if let Some(addr_sender) = addr_sender {
                    addr_sender.send(server.local_addr()).unwrap();
                }
                server
            }
        };

        server.with_graceful_shutdown(shutdown).await
    }
}

#[derive(Debug, Clone)]
struct State {
    solver: Solver,
    shared: Arc<SharedState>,
}

impl State {
    fn solver(&self) -> &Solver {
        &self.solver
    }

    fn simulator(&self) -> &Simulator {
        &self.shared.simulator
    }

    fn ethereum(&self) -> &Ethereum {
        &self.shared.eth
    }

    fn now(&self) -> infra::time::Now {
        self.shared.now
    }

    fn quote_config(&self) -> &competition::quote::Config {
        &self.shared.quote_config
    }
}

/// State which is shared among all multiplexed solvers.
#[derive(Debug)]
struct SharedState {
    simulator: Simulator,
    eth: Ethereum,
    now: infra::time::Now,
    quote_config: competition::quote::Config,
}
