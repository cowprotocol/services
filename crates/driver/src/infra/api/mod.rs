use {
    crate::{
        domain::{self, Mempools},
        infra::{self, liquidity, solver::Solver, tokens, Ethereum, Simulator},
    },
    error::Error,
    futures::Future,
    std::{net::SocketAddr, sync::Arc},
    tokio::sync::oneshot,
};

mod error;
mod routes;

const REQUEST_BODY_LIMIT: usize = 10 * 1024 * 1024;

pub struct Api {
    pub solvers: Vec<Solver>,
    pub liquidity: liquidity::Fetcher,
    pub simulator: Simulator,
    pub eth: Ethereum,
    pub mempools: Mempools,
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

        let tokens = tokens::Fetcher::new(self.eth.clone());
        let pre_processor = domain::competition::AuctionProcessor::new(Arc::new(self.eth.clone()));

        // Add the metrics endpoint.
        app = routes::metrics(app);

        // Multiplex each solver as part of the API. Multiple solvers are multiplexed
        // on the same driver so only one liquidity collector collects the liquidity
        // for all of them. This is important because liquidity collection is
        // computationally expensive for the Ethereum node.
        for solver in self.solvers {
            let name = solver.name().clone();
            let router = axum::Router::new();
            let router = routes::info(router);
            let router = routes::quote(router);
            let router = routes::solve(router);
            let router = routes::reveal(router);
            let router = routes::settle(router);
            let router = router.with_state(State(Arc::new(Inner {
                eth: self.eth.clone(),
                solver: solver.clone(),
                competition: domain::Competition {
                    solver,
                    eth: self.eth.clone(),
                    liquidity: self.liquidity.clone(),
                    simulator: self.simulator.clone(),
                    mempools: self.mempools.clone(),
                    settlement: Default::default(),
                },
                liquidity: self.liquidity.clone(),
                tokens: tokens.clone(),
                pre_processor: pre_processor.clone(),
            })));
            let path = format!("/{name}");
            infra::observe::mounting_solver(&name, &path);
            app = app.nest(&path, router);
        }

        let make_svc = observe::make_service_with_task_local_storage!(app);

        // Start the server.
        let server = axum::Server::bind(&self.addr).serve(make_svc);
        tracing::info!(port = server.local_addr().port(), "serving driver");
        if let Some(addr_sender) = self.addr_sender {
            addr_sender.send(server.local_addr()).unwrap();
        }
        server.with_graceful_shutdown(shutdown).await
    }
}

#[derive(Clone)]
struct State(Arc<Inner>);

impl State {
    fn eth(&self) -> &Ethereum {
        &self.0.eth
    }

    fn solver(&self) -> &Solver {
        &self.0.solver
    }

    fn competition(&self) -> &domain::Competition {
        &self.0.competition
    }

    fn liquidity(&self) -> &liquidity::Fetcher {
        &self.0.liquidity
    }

    fn tokens(&self) -> &tokens::Fetcher {
        &self.0.tokens
    }

    fn pre_processor(&self) -> &domain::competition::AuctionProcessor {
        &self.0.pre_processor
    }
}

struct Inner {
    eth: Ethereum,
    solver: Solver,
    competition: domain::Competition,
    liquidity: liquidity::Fetcher,
    tokens: tokens::Fetcher,
    pre_processor: domain::competition::AuctionProcessor,
}
