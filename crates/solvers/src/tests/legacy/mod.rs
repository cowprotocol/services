use {
    crate::tests,
    std::{
        net::SocketAddr,
        sync::{Arc, Mutex},
    },
};

mod attaching_approvals;
mod jit_order;
mod market_order;

#[derive(Debug, Clone)]
pub struct Expectation {
    pub req: serde_json::Value,
    pub res: serde_json::Value,
}

/// Set up an HTTP server exposing a solver API and acting as a solver mock.
pub async fn setup(expectations: Vec<Expectation>) -> SocketAddr {
    let state = Arc::new(Mutex::new(expectations));
    let app = axum::Router::new()
        .route(
            "/solve",
            axum::routing::post(
                |axum::extract::State(state): axum::extract::State<State>,
                 axum::extract::Json(req): axum::extract::Json<serde_json::Value>| async move {
                    let mut state = state.0.lock().unwrap();
                    assert!(
                        !state.is_empty(),
                        "got another solve request, but didn't expect any more"
                    );
                    let Expectation {
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
    addr
}

#[derive(Debug, Clone)]
struct State(Arc<Mutex<Vec<Expectation>>>);

/// Creates a legacy solver configuration for the specified host.
pub fn config(solver_addr: &SocketAddr) -> tests::Config {
    tests::Config::String(format!(
        r"
solver-name = 'legacy_solver'
endpoint = 'http://{solver_addr}/solve'
chain-id = '1'
        ",
    ))
}
