//! Mock solver for testing purposes. It returns a custom solution.

use {
    crate::setup::solver::shutdown_signal,
    axum::Json,
    futures::{FutureExt, future::BoxFuture},
    reqwest::Url,
    solvers_dto::{
        auction::Auction,
        solution::{Solution, Solutions},
    },
    std::sync::{Arc, Mutex, MutexGuard},
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
    solution: Arc<Mutex<SolutionFuture>>,
}

type SolutionFuture = Arc<dyn Fn() -> BoxFuture<'static, Option<Solution>> + Send + Sync>;

impl Mock {
    /// Instructs the solver to return a new solution from now on.
    pub fn configure_solution(&self, solution: Option<Solution>) {
        *self.state.solution.lock().unwrap() = Arc::new(move || {
            let solution = solution.clone();
            async move { solution }.boxed()
        });
    }

    /// Instructs the solver to return a new solution from now on.
    pub fn configure_solution_async(&self, solution: SolutionFuture) {
        *self.state.solution.lock().unwrap() = solution;
    }

    /// Returns all the auctions received by the solver
    pub fn get_auctions(&self) -> MutexGuard<'_, Vec<Auction>> {
        self.state.auctions.lock().unwrap()
    }
}

impl Default for Mock {
    fn default() -> Self {
        let state = State {
            solution: Arc::new(Mutex::new(Arc::new(|| async { None }.boxed()))),
            auctions: Arc::new(Mutex::new(vec![])),
        };

        let app = axum::Router::new()
            .route("/solve", axum::routing::post(solve))
            .with_state(state.clone());

        let (tx, rx) = std::sync::mpsc::channel();
        tokio::task::spawn(async move {
            let listener = tokio::net::TcpListener::bind("0.0.0.0:0").await.unwrap();
            let local_addr = listener.local_addr().unwrap();
            tx.send(local_addr).unwrap();
            axum::serve(listener, app)
                .with_graceful_shutdown(shutdown_signal())
                .await
                .unwrap();
        });

        let local_addr = rx.recv().unwrap();
        Mock {
            state,
            url: format!("http://{}", local_addr).parse().unwrap(),
        }
    }
}

async fn solve(
    state: axum::extract::State<State>,
    Json(auction): Json<Auction>,
) -> (axum::http::StatusCode, Json<Solutions>) {
    let auction_id = auction.id.unwrap_or_default();
    state.auctions.lock().unwrap().push(auction);
    let solutions: Vec<_> = {
        let solution_generator = state.solution.lock().unwrap().clone();
        solution_generator().await.into_iter().collect()
    };
    let solutions = Solutions { solutions };
    tracing::trace!(?auction_id, ?solutions, "/solve");
    (axum::http::StatusCode::OK, Json(solutions))
}
