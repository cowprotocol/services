//! Serve a solver engine API.

use {
    crate::domain::solver::Solver,
    serde_json::Error,
    std::{future::Future, net::SocketAddr, sync::Arc},
    tokio::sync::oneshot,
    utoipa::OpenApi,
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

// migrate to utoipauto once the issue is solved https://github.com/ProbablyClem/utoipauto/issues/23
pub fn generate_openapi_json() -> Result<String, Error> {
    #[derive(OpenApi)]
    #[openapi(
        paths(routes::solve::solve, routes::notify::notify,),
        components(schemas(
            solvers_dto::auction::Auction,
            solvers_dto::auction::TokenInfo,
            solvers_dto::auction::NativePrice,
            solvers_dto::auction::DateTime,
            solvers_dto::auction::Liquidity,
            solvers_dto::auction::LiquidityParameters,
            solvers_dto::auction::ConstantProductPool,
            solvers_dto::auction::WeightedProductPool,
            solvers_dto::auction::StablePool,
            solvers_dto::auction::ConcentratedLiquidityPool,
            solvers_dto::auction::ForeignLimitOrder,
            solvers_dto::auction::TokenReserve,
            solvers_dto::auction::BalancerPoolId,
            solvers_dto::auction::Decimal,
            solvers_dto::auction::U256Schema,
            solvers_dto::auction::U128,
            solvers_dto::auction::I128,
            solvers_dto::auction::I32,
            solvers_dto::auction::BigInt,
            solvers_dto::auction::Order,
            solvers_dto::auction::OrderUid,
            solvers_dto::auction::FeePolicy,
            solvers_dto::auction::Quote,
            solvers_dto::auction::OrderClass,
            solvers_dto::auction::OrderKind,
            solvers_dto::auction::SurplusFee,
            solvers_dto::auction::PriceImprovement,
            solvers_dto::auction::VolumeFee,
            solvers_dto::auction::SellTokenSource,
            solvers_dto::auction::InteractionData,
            solvers_dto::auction::BuyTokenDestination,
            solvers_dto::auction::SellTokenSource,
            solvers_dto::auction::LegacySigningScheme,
            solvers_dto::solution::Solution,
            solvers_dto::solution::Interaction,
            solvers_dto::solution::CustomInteraction,
            solvers_dto::solution::LiquidityInteraction,
            solvers_dto::solution::Allowance,
            solvers_dto::solution::Asset,
            solvers_dto::solution::Trade,
            solvers_dto::solution::Fulfillment,
            solvers_dto::solution::JitTrade,
            solvers_dto::solution::JitOrder,
            solvers_dto::solution::BuyTokenBalance,
            solvers_dto::solution::SellTokenBalance,
            solvers_dto::solution::SigningScheme,
            solvers_dto::common::Address,
            solvers_dto::common::AppData,
            solvers_dto::common::Signature,
            solvers_dto::common::TokenAmount,
            solvers_dto::common::Token,
        )),
        info(
            description = "The API implemented by solver engines interacting with the reference \
                           driver implementation.",
            title = "Solver Engine API",
            version = "0.1.0",
            license(name = "MIT OR Apache-2.0")
        )
    )]
    pub struct ApiDoc;

    ApiDoc::openapi().to_pretty_json()
}
