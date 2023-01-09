//! Serve a solver engine API.

use crate::domain::{baseline, eth};
use std::{future::Future, net::SocketAddr, num::NonZeroUsize};

pub mod dto;

pub struct Api {
    pub addr: SocketAddr,
}

impl Api {
    pub async fn serve(
        self,
        shutdown: impl Future<Output = ()> + Send + 'static,
    ) -> Result<(), hyper::Error> {
        // Add middleware.
        let app = axum::Router::new()
            .layer(
                tower::ServiceBuilder::new().layer(tower_http::trace::TraceLayer::new_for_http()),
            )
            .route("/", axum::routing::post(solve));

        // Start the server.
        axum::Server::bind(&self.addr)
            .serve(app.into_make_service())
            .with_graceful_shutdown(shutdown)
            .await
    }
}

async fn solve(
    auction: axum::extract::Json<dto::Auction>,
) -> (
    axum::http::StatusCode,
    axum::response::Json<dto::Response<dto::Solution>>,
) {
    let auction = match auction.to_domain() {
        Ok(value) => value,
        Err(err) => {
            return (
                axum::http::StatusCode::OK,
                axum::response::Json(dto::Response::Err(err)),
            )
        }
    };
    let solver = baseline::Baseline {
        weth: eth::WethAddress(
            "0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2"
                .parse()
                .unwrap(),
        ),
        base_tokens: Default::default(),
        max_hops: NonZeroUsize::new(2).unwrap(),
    };
    let solution = solver
        .solve(&auction)
        .into_iter()
        .next()
        .map(|solution| dto::Solution::from_domain(&solution))
        .unwrap_or_else(dto::Solution::trivial);

    (
        axum::http::StatusCode::OK,
        axum::response::Json(dto::Response::Ok(solution)),
    )
}
