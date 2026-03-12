//! Mock solver for testing purposes. It returns a custom solution.

use {
    crate::setup::solver::shutdown_signal,
    axum::Json,
    futures::{FutureExt, future::BoxFuture},
    reqwest::Url,
    solvers_dto::{
        auction::Auction,
        solution::{Solution, SolverResponse},
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
    /// The currently configured response to return.
    response: Arc<Mutex<ResponseFuture>>,
}

type SolutionFuture = Arc<dyn Fn() -> BoxFuture<'static, Option<Solution>> + Send + Sync>;
type ResponseFuture = Arc<dyn Fn() -> BoxFuture<'static, SolverResponse> + Send + Sync>;

impl Mock {
    /// Instructs the solver to return a new response from now on.
    pub fn configure_response(&self, response: SolverResponse) {
        *self.state.response.lock().unwrap() = Arc::new(move || {
            let response = response.clone();
            async move { response }.boxed()
        });
    }

    /// Instructs the solver to return a new response from now on.
    pub fn configure_response_async(&self, response: ResponseFuture) {
        *self.state.response.lock().unwrap() = response;
    }

    /// Instructs the solver to return a new solution from now on.
    pub fn configure_solution(&self, solution: Option<Solution>) {
        self.configure_response(SolverResponse::Solutions {
            solutions: solution.into_iter().collect(),
        });
    }

    /// Instructs the solver to return a new solution from now on.
    pub fn configure_solution_async(&self, solution: SolutionFuture) {
        self.configure_response_async(Arc::new(move || {
            let solution = solution.clone();
            async move {
                SolverResponse::Solutions {
                    solutions: solution().await.into_iter().collect(),
                }
            }
            .boxed()
        }));
    }

    /// Returns all the auctions received by the solver
    pub fn get_auctions(&self) -> MutexGuard<'_, Vec<Auction>> {
        self.state.auctions.lock().unwrap()
    }
}

impl Mock {
    pub async fn new() -> Self {
        let state = State {
            response: Arc::new(Mutex::new(Arc::new(|| {
                async {
                    SolverResponse::Solutions {
                        solutions: Vec::new(),
                    }
                }
                .boxed()
            }))),
            auctions: Arc::new(Mutex::new(vec![])),
        };

        let app = axum::Router::new()
            .route("/solve", axum::routing::post(solve))
            .with_state(state.clone());

        let listener = tokio::net::TcpListener::bind("0.0.0.0:0").await.unwrap();
        let local_addr = listener.local_addr().unwrap();

        tokio::task::spawn(async move {
            axum::serve(listener, app)
                .with_graceful_shutdown(shutdown_signal())
                .await
                .unwrap();
        });

        Mock {
            state,
            url: format!("http://{}", local_addr).parse().unwrap(),
        }
    }
}

async fn solve(
    state: axum::extract::State<State>,
    Json(auction): Json<Auction>,
) -> (axum::http::StatusCode, Json<SolverResponse>) {
    let auction_id = auction.id.unwrap_or_default();
    state.auctions.lock().unwrap().push(auction);
    let response = {
        let response_generator = state.response.lock().unwrap().clone();
        response_generator().await
    };
    tracing::trace!(?auction_id, ?response, "/solve");
    (axum::http::StatusCode::OK, Json(response))
}
