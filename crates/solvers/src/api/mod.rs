//! Serve a solver engine API.

use {
    crate::domain::solver::Solver,
    std::{future::Future, net::SocketAddr, sync::Arc},
    tokio::sync::oneshot,
    tracing::Instrument,
};

pub mod dto;

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
            .route("/", axum::routing::post(solve))
            .layer(
                tower::ServiceBuilder::new().layer(tower_http::trace::TraceLayer::new_for_http()),
            )
            .with_state(Arc::new(self.solver));

        let make_svc = shared::make_service_with_task_local_storage!(app);

        let server = axum::Server::bind(&self.addr).serve(make_svc);
        if let Some(bind) = bind {
            let _ = bind.send(server.local_addr());
        }

        server.with_graceful_shutdown(shutdown).await
    }
}

async fn solve(
    state: axum::extract::State<Arc<Solver>>,
    auction: axum::extract::Json<dto::Auction>,
) -> (
    axum::http::StatusCode,
    axum::response::Json<dto::Response<dto::Solutions>>,
) {
    let handle_request = async {
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

        tracing::trace!(?auction);

        let solutions = state
            .solve(auction)
            .await
            .into_iter()
            .next()
            .map(|solution| dto::Solutions::from_domain(&[solution]))
            .unwrap_or_default();

        tracing::trace!(?solutions);

        (
            axum::http::StatusCode::OK,
            axum::response::Json(dto::Response::Ok(solutions)),
        )
    };

    handle_request
        .instrument(tracing::info_span!("/solve"))
        .await
}
