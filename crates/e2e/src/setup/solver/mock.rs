//! Mock solver for testing purposes. It returns a custom solution.

use {
    crate::setup::solver::shutdown_signal,
    axum::Json,
    reqwest::Url,
    solvers_dto::{
        auction::Auction,
        solution::{Solution, Solutions},
    },
    std::sync::{Arc, Mutex},
    warp::hyper,
};

/// A solver that does not implement any solving logic itself and instead simply
/// forwards a single hardcoded solution.
pub struct Mock {
    /// The currently configured solution to return.
    solution: Arc<Mutex<Option<Solution>>>,
    /// Under which URL the solver is reachable by a driver.
    pub url: Url,
}

impl Mock {
    /// Instructs the solver to return a new solution from now on.
    pub fn configure_solution(&self, solution: Option<Solution>) {
        *self.solution.lock().unwrap() = solution;
    }
}

impl Default for Mock {
    fn default() -> Self {
        let solution = Arc::new(Mutex::new(None));

        let app = axum::Router::new()
            .route("/solve", axum::routing::post(solve))
            .with_state(solution.clone());

        let make_svc = observe::make_service_with_task_local_storage!(app);
        let server = axum::Server::bind(&"0.0.0.0:0".parse().unwrap()).serve(make_svc);

        let mock = Mock {
            solution,
            url: format!("http://{}", server.local_addr()).parse().unwrap(),
        };

        tokio::task::spawn(server.with_graceful_shutdown(shutdown_signal()));

        mock
    }
}

async fn solve(
    state: axum::extract::State<Arc<Mutex<Option<Solution>>>>,
    Json(auction): Json<Auction>,
) -> (axum::http::StatusCode, Json<Solutions>) {
    let auction_id = auction.id.unwrap_or_default();
    let solutions = state.lock().unwrap().iter().cloned().collect();
    let solutions = Solutions { solutions };
    tracing::trace!(?auction_id, ?solutions, "/solve");
    (axum::http::StatusCode::OK, Json(solutions))
}
