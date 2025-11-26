use {
    crate::solver::EulerSolver,
    axum::{
        Router,
        extract::{Json, State},
        http::StatusCode,
        response::IntoResponse,
        routing::{get, post},
    },
    serde::{Deserialize, Serialize},
    solvers_dto::{auction::Auction, solution::Solutions},
    std::{future::Future, net::SocketAddr, sync::Arc},
    tokio::sync::oneshot,
    tower_http::trace::{DefaultMakeSpan, DefaultOnResponse, TraceLayer},
    tracing::Level,
};

pub struct Api {
    pub addr: SocketAddr,
    pub solver: EulerSolver,
}

impl Api {
    pub async fn serve(
        self,
        bind: Option<oneshot::Sender<SocketAddr>>,
        shutdown: impl Future<Output = ()> + Send + 'static,
    ) -> anyhow::Result<()> {
        let app = Router::new()
            .route("/solve", post(solve))
            .route("/reveal", post(reveal))
            .route("/settle", post(settle))
            .route("/healthz", get(healthz))
            .layer(
                TraceLayer::new_for_http()
                    .make_span_with(DefaultMakeSpan::new().level(Level::INFO))
                    .on_response(DefaultOnResponse::new().level(Level::INFO)),
            )
            .with_state(Arc::new(self.solver));

        let server = axum::Server::bind(&self.addr).serve(app.into_make_service());
        if let Some(bind) = bind {
            let _ = bind.send(server.local_addr());
        }

        server.with_graceful_shutdown(shutdown).await?;

        Ok(())
    }
}

#[derive(Debug, Serialize)]
#[serde(untagged)]
pub enum Response<T> {
    Ok(T),
    Err(ErrorResponse),
}

#[derive(Debug, Serialize)]
pub struct ErrorResponse {
    pub error: String,
}

// POST /solve
async fn solve(
    State(solver): State<Arc<EulerSolver>>,
    Json(auction): Json<Auction>,
) -> impl IntoResponse {
    tracing::info!("Received auction with {} orders", auction.orders.len());

    match solver.solve(&auction).await {
        Ok(results) => {
            let solutions = Solutions {
                solutions: results.into_iter().map(|(s, _)| s).collect(),
            };
            tracing::info!("Generated solution {:?}", solutions.solutions);
            (StatusCode::OK, Json(Response::Ok(solutions)))
        }
        Err(err) => {
            tracing::error!("Failed to solve: {}", err);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(Response::Err(ErrorResponse {
                    error: err.to_string(),
                })),
            )
        }
    }
}

// POST /reveal
// For now, this is a stub. In a full implementation, this would return settlement calldata
#[derive(Debug, Deserialize)]
struct RevealRequest {
    solution_id: u64,
    auction_id: Option<i64>,
}

#[derive(Debug, Serialize)]
struct RevealResponse {
    calldata: String,
}

async fn reveal(Json(request): Json<RevealRequest>) -> impl IntoResponse {
    tracing::info!(
        "Reveal request for solution {} in auction {:?}",
        request.solution_id,
        request.auction_id
    );

    // TODO: Implement actual calldata generation
    // For now, return empty calldata
    (
        StatusCode::OK,
        Json(RevealResponse {
            calldata: "0x".to_string(),
        }),
    )
}

// POST /settle
// For now, this is a stub. The driver will handle actual settlement
#[derive(Debug, Deserialize)]
struct SettleRequest {
    solution_id: u64,
    auction_id: Option<i64>,
}

async fn settle(Json(request): Json<SettleRequest>) -> impl IntoResponse {
    tracing::info!(
        "Settle request for solution {} in auction {:?}",
        request.solution_id,
        request.auction_id
    );

    // Acknowledge the settle request
    StatusCode::OK
}

// GET /healthz
async fn healthz() -> impl IntoResponse {
    StatusCode::OK
}
