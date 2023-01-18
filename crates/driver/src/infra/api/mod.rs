use {
    crate::{
        domain::{self, competition},
        infra::{self, time, Ethereum, Mempool, Simulator},
        solver::Solver,
    },
    futures::Future,
    std::{net::SocketAddr, sync::Arc},
    tokio::sync::oneshot,
};

mod info;
mod quote;
mod settle;
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
    pub mempool: Mempool,
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
        for solver in self.solvers {
            let name = solver.name().clone();
            let router = axum::Router::new();
            let router = info::route(router);
            let router = quote::route(router);
            let router = solve::route(router);
            let router = settle::route(router);
            let router = router.with_state(State(Arc::new(Inner {
                solver: solver.clone(),
                competition: domain::Competition {
                    solver,
                    eth: self.eth.clone(),
                    simulator: self.simulator.clone(),
                    now: self.now,
                    mempools: vec![self.mempool.clone()],
                    settlement: Default::default(),
                },
                quote_config: self.quote_config.clone(),
                now: self.now,
            })));
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
struct State(Arc<Inner>);

impl State {
    fn solver(&self) -> &Solver {
        &self.0.solver
    }

    fn competition(&self) -> &domain::Competition {
        &self.0.competition
    }

    fn quote_config(&self) -> &competition::quote::Config {
        &self.0.quote_config
    }

    fn now(&self) -> time::Now {
        self.0.now
    }
}

#[derive(Debug)]
struct Inner {
    solver: Solver,
    competition: domain::Competition,
    quote_config: competition::quote::Config,
    now: time::Now,
}
