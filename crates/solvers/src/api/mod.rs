//! Serve a solver engine API.

use crate::domain::baseline;
use std::{future::Future, net::SocketAddr, sync::Arc};
use tokio::sync::oneshot;

pub mod dto;

pub struct Api {
    pub addr: SocketAddr,
    pub solver: baseline::Baseline,
}

impl Api {
    pub async fn serve(
        self,
        bind: Option<oneshot::Sender<SocketAddr>>,
        shutdown: impl Future<Output = ()> + Send + 'static,
    ) -> Result<(), hyper::Error> {
        let app = axum::Router::new()
            .route("/", axum::routing::post(solve))
            .layer(
                tower::ServiceBuilder::new().layer(tower_http::trace::TraceLayer::new_for_http()),
            )
            .with_state(Arc::new(self.solver));

        let server = axum::Server::bind(&self.addr).serve(app.into_make_service());
        if let Some(bind) = bind {
            let _ = bind.send(server.local_addr());
        }

        server.with_graceful_shutdown(shutdown).await
    }
}

async fn solve(
    state: axum::extract::State<Arc<baseline::Baseline>>,
    auction: axum::extract::Json<dto::Auction>,
) -> (
    axum::http::StatusCode,
    axum::response::Json<dto::Response<dto::Solution>>,
) {
    let auction = match auction.to_domain() {
        Ok(value) => value,
        Err(err) => {
            tracing::warn!(?err, "invalid auction");
            return (
                axum::http::StatusCode::BAD_REQUEST,
                axum::response::Json(dto::Response::Err(err)),
            );
        }
    };

    let solution = state
        .solve(&auction)
        .first()
        .map(dto::Solution::from_domain)
        .unwrap_or_else(dto::Solution::trivial);

    (
        axum::http::StatusCode::OK,
        axum::response::Json(dto::Response::Ok(solution)),
    )
}
