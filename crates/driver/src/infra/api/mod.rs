use {
    crate::{
        domain::{self, competition},
        infra::{self, liquidity, time, Ethereum, Mempool, Simulator},
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

pub struct Api {
    pub solvers: Vec<Solver>,
    pub liquidity: liquidity::Fetcher,
    pub simulator: Simulator,
    pub eth: Ethereum,
    pub mempools: Vec<Mempool>,
    pub now: infra::time::Now,
    pub quote_config: competition::quote::Config,
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
                liquidity: self.liquidity.clone(),
                competition: domain::Competition {
                    solver,
                    eth: self.eth.clone(),
                    simulator: self.simulator.clone(),
                    now: self.now,
                    mempools: self.mempools.clone(),
                    settlement: Default::default(),
                },
                quote_config: self.quote_config.clone(),
                now: self.now,
            })));
            app = app.nest(&format!("/{name}"), router);
        }

        // Start the server.
        let server = axum::Server::bind(&self.addr).serve(app.into_make_service());
        if let Some(addr_sender) = self.addr_sender {
            addr_sender.send(server.local_addr()).unwrap();
        }
        server.with_graceful_shutdown(shutdown).await
    }
}

#[derive(Debug, Clone)]
struct State(Arc<Inner>);

impl State {
    fn solver(&self) -> &Solver {
        &self.0.solver
    }

    fn liquidity(&self) -> &liquidity::Fetcher {
        &self.0.liquidity
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
    liquidity: liquidity::Fetcher,
    competition: domain::Competition,
    quote_config: competition::quote::Config,
    now: time::Now,
}
