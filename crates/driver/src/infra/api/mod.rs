use {
    crate::{
        domain::{
            self,
            competition::{bad_tokens, order::app_data::AppDataRetriever},
            Mempools,
        },
        infra::{
            self,
            config::file::OrderPriorityStrategy,
            liquidity,
            solver::{Solver, Timeouts},
            tokens,
            Ethereum,
            Simulator,
        },
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
    pub bad_token_detector: bad_tokens::simulation::Detector,
    /// If this channel is specified, the bound address will be sent to it. This
    /// allows the driver to bind to 0.0.0.0:0 during testing.
    pub addr_sender: Option<oneshot::Sender<SocketAddr>>,
}

impl Api {
    pub async fn serve(
        self,
        shutdown: impl Future<Output = ()> + Send + 'static,
        order_priority_strategies: Vec<OrderPriorityStrategy>,
        app_data_retriever: Option<AppDataRetriever>,
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
        let pre_processor = domain::competition::AuctionProcessor::new(
            &self.eth,
            order_priority_strategies,
            app_data_retriever,
        );

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

            let bad_token_config = solver.bad_token_detection();
            let mut bad_tokens =
                bad_tokens::Detector::new(bad_token_config.tokens_supported.clone());
            if bad_token_config.enable_simulation_strategy {
                bad_tokens.with_simulation_detector(self.bad_token_detector.clone());
            }

            if bad_token_config.enable_metrics_strategy {
                bad_tokens.with_metrics_detector(bad_tokens::metrics::Detector::new(
                    bad_token_config.metrics_strategy_failure_ratio,
                    bad_token_config.metrics_strategy_required_measurements,
                    bad_token_config.metrics_strategy_log_only,
                    bad_token_config.metrics_strategy_token_freeze_time,
                    name.clone(),
                ));
            }

            let router = router.with_state(State(Arc::new(Inner {
                eth: self.eth.clone(),
                solver: solver.clone(),
                competition: domain::Competition::new(
                    solver,
                    self.eth.clone(),
                    self.liquidity.clone(),
                    self.simulator.clone(),
                    self.mempools.clone(),
                    Arc::new(bad_tokens),
                ),
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

        let make_svc = observe::make_service_with_request_tracing!(app);

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
    competition: Arc<domain::Competition>,
    liquidity: liquidity::Fetcher,
    tokens: tokens::Fetcher,
    pre_processor: domain::competition::AuctionProcessor,
}
