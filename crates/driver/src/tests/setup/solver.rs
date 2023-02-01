use std::{
    net::SocketAddr,
    sync::{Arc, Mutex},
};

/// Configuration for mocking a solver.
#[derive(Debug, Default)]
pub struct Config {
    /// The auctions and solutions that the solver should expect and respond
    /// with.
    pub solve: Vec<Solve>,
    pub name: String,
    pub absolute_slippage: String,
    pub relative_slippage: String,
    pub address: String,
    pub private_key: String,
}

#[derive(Debug, Clone)]
pub struct Solve {
    pub req: serde_json::Value,
    pub res: serde_json::Value,
}

/// A solver that was started as part of the setup.
#[derive(Debug)]
pub struct Solver {
    pub config: Config,
    pub addr: SocketAddr,
}

/// Set up an HTTP server exposing a solver API and acting as a solver mock.
pub async fn setup(config: Config) -> Solver {
    let state = Arc::new(Mutex::new(config.solve.clone()));
    let app = axum::Router::new()
        .route(
            "/",
            axum::routing::post(
                |axum::extract::State(state): axum::extract::State<State>,
                 axum::extract::Json(req): axum::extract::Json<serde_json::Value>| async move {
                    let mut state = state.0.lock().unwrap();
                    assert!(
                        !state.is_empty(),
                        "got another solve request, but didn't expect any more"
                    );
                    let Solve {
                        req: expected_req,
                        res,
                    } = state.pop().unwrap();
                    assert_eq!(req, expected_req, "solve request has unexpected body");
                    axum::response::Json(res)
                },
            ),
        )
        .with_state(State(state));
    let server = axum::Server::bind(&"0.0.0.0:0".parse().unwrap()).serve(app.into_make_service());
    let addr = server.local_addr();
    tokio::spawn(async move { server.await.unwrap() });
    Solver { config, addr }
}

#[derive(Debug, Clone)]
struct State(Arc<Mutex<Vec<Solve>>>);
