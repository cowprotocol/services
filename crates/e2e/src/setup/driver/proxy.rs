use {
    axum::{Json, response::IntoResponse},
    reqwest::Url,
    std::sync::{Arc, Mutex},
    warp::hyper,
};

/// A proxy driver that forwards requests to an upstream driver with some
/// additional logic.
pub struct Proxy {
    /// Proxy driver shareable state.
    state: State,
    /// Under which URL the driver is reachable by an autopilot.
    pub url: String,
}

type DecisionFn = dyn Fn(usize) -> bool + Send + Sync;

#[derive(Clone)]
pub struct State {
    /// Base URL for the upstream driver.
    upstream_base_url: Arc<Mutex<Url>>,
    /// Counter for `/settle` requests.
    settle_counter: Arc<Mutex<usize>>,
    /// Function that decides whether to return an error or redirect
    settle_decision_fn: Arc<Mutex<Box<DecisionFn>>>,
    /// Counter for `/solve` requests.
    solve_counter: Arc<Mutex<usize>>,
    /// Function that decides whether to return an error or redirect
    solve_decision_fn: Arc<Mutex<Box<DecisionFn>>>,
}

impl Default for Proxy {
    fn default() -> Self {
        let state = State {
            upstream_base_url: Arc::new(Mutex::new(
                "http://localhost:11088/test_solver/".parse().unwrap(),
            )),
            settle_counter: Default::default(),
            // By default, just redirect to the upstream.
            settle_decision_fn: Arc::new(Mutex::new(Box::new(|_counter| false))),
            solve_counter: Default::default(),
            solve_decision_fn: Arc::new(Mutex::new(Box::new(|_counter| false))),
        };

        let app = axum::Router::new()
            .route("/settle", axum::routing::post(settle))
            .route("/solve", axum::routing::post(solve))
            .route("/:path", axum::routing::any(fallback))
            .with_state(state.clone());

        let make_svc = observe::make_service_with_request_tracing!(app);
        let server = axum::Server::bind(&"0.0.0.0:0".parse().unwrap()).serve(make_svc);

        let proxy = Proxy {
            state,
            url: format!("http://{}", server.local_addr()).parse().unwrap(),
        };

        tokio::task::spawn(server.with_graceful_shutdown(super::shutdown_signal()));

        proxy
    }
}

impl Proxy {
    /// Sets the base URL for the upstream driver.
    pub fn set_upstream_base_url(&self, url: Url) {
        *self.state.upstream_base_url.lock().unwrap() = url;
    }

    pub fn error_on_settle_when<F>(&self, func: F)
    where
        F: Fn(usize) -> bool + Send + Sync + 'static,
    {
        *self.state.settle_decision_fn.lock().unwrap() = Box::new(func);
    }

    pub fn error_on_solve_when<F>(&self, func: F)
    where
        F: Fn(usize) -> bool + Send + Sync + 'static,
    {
        *self.state.solve_decision_fn.lock().unwrap() = Box::new(func);
    }

    pub fn get_settle_counter(&self) -> usize {
        *self.state.settle_counter.lock().unwrap()
    }
}

async fn settle(axum::extract::State(state): axum::extract::State<State>) -> impl IntoResponse {
    {
        let mut counter = state.settle_counter.lock().unwrap();
        *counter += 1;
        let should_error = state.settle_decision_fn.lock().unwrap()(*counter);
        if should_error {
            tracing::debug!(?counter, "returning error for /settle request");
            return (
                axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({ "error": "Mocked failure" })),
            )
                .into_response();
        }
    }

    let upstream_url = state
        .upstream_base_url
        .lock()
        .unwrap()
        .join("settle")
        .expect("build URL")
        .to_string();
    tracing::debug!(?upstream_url, "redirecting to the upstream URL");
    axum::response::Redirect::temporary(&upstream_url).into_response()
}

async fn solve(axum::extract::State(state): axum::extract::State<State>) -> impl IntoResponse {
    {
        let mut counter = state.solve_counter.lock().unwrap();
        *counter += 1;
        let should_error = state.solve_decision_fn.lock().unwrap()(*counter);
        if should_error {
            tracing::debug!(?counter, "returning error for /solve request");
            return (
                axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({ "error": "Mocked failure" })),
            )
                .into_response();
        }
    }

    let upstream_url = state
        .upstream_base_url
        .lock()
        .unwrap()
        .join("solve")
        .expect("build URL")
        .to_string();
    tracing::debug!(?upstream_url, "redirecting to the upstream URL");
    axum::response::Redirect::temporary(&upstream_url).into_response()
}

/// Redirects any request to the upstream.
async fn fallback(
    axum::extract::State(state): axum::extract::State<State>,
    axum::extract::Path(path): axum::extract::Path<String>,
) -> impl IntoResponse {
    let upstream_url = state
        .upstream_base_url
        .lock()
        .unwrap()
        .join(&path)
        .expect("build URL");
    tracing::debug!(?upstream_url, "redirecting to the upstream URL");
    axum::response::Redirect::temporary(upstream_url.as_str()).into_response()
}
