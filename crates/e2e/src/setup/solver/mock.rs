//! Mock solver for testing purposes. It returns a custom solution.

use {
    crate::setup::solver::shutdown_signal,
    axum::Json,
    reqwest::Url,
    solvers_dto::{
        auction::Auction,
        solution::{Solution, Solutions},
    },
    std::sync::{Arc, Mutex, MutexGuard},
    tokio::net::TcpListener,
};

/// A solver that does not implement any solving logic itself and instead simply
/// forwards a single hardcoded solution.
pub struct Mock {
    /// Mock solver shareable state
    state: State,
    /// Under which URL the solver is reachable by a driver.
    pub url: Url,
}

#[derive(Clone)]
pub struct State {
    /// In-memory set of the auctions received by the solver.
    auctions: Arc<Mutex<Vec<Auction>>>,
    /// The currently configured solution to return.
    solution: Arc<Mutex<Option<Solution>>>,
}

impl Mock {
    /// Instructs the solver to return a new solution from now on.
    pub fn configure_solution(&self, solution: Option<Solution>) {
        *self.state.solution.lock().unwrap() = solution;
    }

    /// Returns all the auctions received by the solver
    pub fn get_auctions(&self) -> MutexGuard<'_, Vec<Auction>> {
        self.state.auctions.lock().unwrap()
    }

    pub async fn new() -> Self {
        let state = State {
            solution: Arc::new(Mutex::new(None)),
            auctions: Arc::new(Mutex::new(vec![])),
        };

        let router: axum::Router<()> = axum::Router::new()
            .route("/solve", axum::routing::post(solve))
            .with_state(state.clone());

        let router = observe::make_service_with_task_local_storage!(router);
        let listener = TcpListener::bind(&"0.0.0.0:0").await.unwrap();

        let mock = Mock {
            state,
            url: format!("http://{}", listener.local_addr().unwrap())
                .parse()
                .unwrap(),
        };
        let server = axum::serve(listener, router).with_graceful_shutdown(shutdown_signal());

        tokio::task::spawn(async move { server.await.unwrap() });

        mock
    }
}

async fn solve(
    state: axum::extract::State<State>,
    Json(auction): Json<Auction>,
) -> (axum::http::StatusCode, Json<Solutions>) {
    let auction_id = auction.id.unwrap_or_default();
    state.auctions.lock().unwrap().push(auction);
    let solutions = state.solution.lock().unwrap().iter().cloned().collect();
    let solutions = Solutions { solutions };
    tracing::trace!(?auction_id, ?solutions, "/solve");
    (axum::http::StatusCode::OK, Json(solutions))
}
