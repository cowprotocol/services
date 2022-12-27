//! Serve a solver engine API.

use std::{future::Future, net::SocketAddr};

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
                tower::ServiceBuilder::new()
                    .layer(tower_http::trace::TraceLayer::new_for_http()),
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
    _auction: axum::extract::Json<dto::Auction>,
) -> axum::response::Json<dto::Solution> {
    axum::response::Json(dto::Solution::trivial())
}
