use {
    crate::{
        domain::{
            self,
            competition::{
                bad_tokens,
                bad_tokens::{Cache, Quality},
            },
            eth,
            Mempools,
        },
        infra::{
            self,
            config::file::{BadTokenDetectionCache, OrderPriorityStrategy},
            liquidity,
            solver::{Solver, Timeouts},
            tokens,
            Ethereum,
            Simulator,
        },
    },
    error::Error,
    futures::Future,
    std::{collections::HashMap, net::SocketAddr, sync::Arc},
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
    pub bad_token_detection_cache: BadTokenDetectionCache,
}

impl Api {
    pub async fn serve(
        self,
        shutdown: impl Future<Output = ()> + Send + 'static,
        order_priority_strategies: Vec<OrderPriorityStrategy>,
    ) -> Result<(), hyper::Error> {
        // Add middleware.
        let mut app = axum::Router::new().layer(
            tower::ServiceBuilder::new()
                .layer(tower_http::limit::RequestBodyLimitLayer::new(
                    REQUEST_BODY_LIMIT,
                ))
                .layer(tower_http::trace::TraceLayer::new_for_http()),
        );

        let tokens = tokens::Fetcher::new(&self.eth);
        let pre_processor =
            domain::competition::AuctionProcessor::new(&self.eth, order_priority_strategies);

        // TODO: create a struct wrapper to handle this under the hood
        let trace_detector = Arc::new(Cache::new(&self.bad_token_detection_cache));

        // Add the metrics and healthz endpoints.
        app = routes::metrics(app);
        app = routes::healthz(app);

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

            let bad_tokens = solver.bad_token_detector().map(|bad_token_detector| {
                // maybe make this as part of the bad token builder?
                let config = bad_token_detector
                    .unsupported_tokens
                    .iter()
                    .map(|token| (eth::TokenAddress::from(*token), Quality::Unsupported))
                    .chain(
                        bad_token_detector
                            .allowed_tokens
                            .iter()
                            .map(|token| (eth::TokenAddress::from(*token), Quality::Supported)),
                    )
                    .collect::<HashMap<_, _>>();

                Arc::new(
                    // maybe do proper builder pattern here?
                    bad_tokens::Detector::default()
                        .with_simulation_detector(&self.eth.clone())
                        .with_config(config)
                        .with_cache(trace_detector.clone()),
                )
            });

            let router = router.with_state(State(Arc::new(Inner {
                eth: self.eth.clone(),
                solver: solver.clone(),
                competition: domain::Competition {
                    solver,
                    eth: self.eth.clone(),
                    liquidity: self.liquidity.clone(),
                    simulator: self.simulator.clone(),
                    mempools: self.mempools.clone(),
                    settlements: Default::default(),
                    bad_tokens,
                },
                liquidity: self.liquidity.clone(),
                tokens: tokens.clone(),
                pre_processor: pre_processor.clone(),
            })));
            let path = format!("/{name}");
            infra::observe::mounting_solver(&name, &path);
            app = app
                .nest(&path, router)
                // axum's default body limit needs to be disabled to not have the default limit on top of our custom limit
                .layer(axum::extract::DefaultBodyLimit::disable());
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

    fn timeouts(&self) -> Timeouts {
        self.0.solver.timeouts()
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
