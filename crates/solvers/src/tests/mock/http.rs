use std::{
    fmt::{self, Debug, Formatter},
    net::SocketAddr,
    sync::{Arc, Mutex},
};

#[derive(Clone)]
pub enum Path {
    Any,
    Exact(String),
    Glob(glob::Pattern),
}

impl Path {
    pub fn exact(s: impl ToString) -> Self {
        Self::Exact(s.to_string())
    }

    pub fn glob(s: impl AsRef<str>) -> Self {
        Self::Glob(glob::Pattern::new(s.as_ref()).unwrap())
    }
}

impl PartialEq<Path> for String {
    fn eq(&self, path: &Path) -> bool {
        match path {
            Path::Any => true,
            Path::Exact(exact) => exact == self,
            Path::Glob(glob) => glob.matches(self),
        }
    }
}

impl Debug for Path {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        match self {
            Path::Any => f.debug_tuple("Any").finish(),
            Path::Exact(exact) => f
                .debug_tuple("Exact")
                .field(&format_args!("{exact}"))
                .finish(),
            Path::Glob(glob) => f
                .debug_tuple("Glob")
                .field(&format_args!("{}", glob.as_str()))
                .finish(),
        }
    }
}

#[derive(Clone, Debug)]
pub enum Expectation {
    Get {
        path: Path,
        res: serde_json::Value,
    },
    Post {
        path: Path,
        req: serde_json::Value,
        res: serde_json::Value,
    },
}

/// Set up an mock external HTTP API.
pub async fn setup(expectations: Vec<Expectation>) -> SocketAddr {
    let state = Arc::new(Mutex::new(expectations));
    let app = axum::Router::new()
        .route(
            "/*path",
            axum::routing::get(
                |axum::extract::State(state),
                 axum::extract::Path(path),
                 axum::extract::RawQuery(query)| async move {
                    axum::response::Json(get(state, Some(path), query))
                },
            )
            .post(
                |axum::extract::State(state),
                 axum::extract::Path(path),
                 axum::extract::RawQuery(query),
                 axum::extract::Json(req)| async move {
                    axum::response::Json(post(state, Some(path), query, req))
                },
            ),
        )
        // Annoying, but `axum` doesn't seem to match `/` with the above route,
        // so explicitly mount `/`.
        .route(
            "/",
            axum::routing::get(
                |axum::extract::State(state), axum::extract::RawQuery(query)| async move {
                    axum::response::Json(get(state, None, query))
                },
            )
            .post(
                |axum::extract::State(state),
                 axum::extract::RawQuery(query),
                 axum::extract::Json(req)| async move {
                    axum::response::Json(post(state, None, query, req))
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

fn get(state: State, path: Option<String>, query: Option<String>) -> serde_json::Value {
    let expectation = state.0.lock().unwrap().pop();
    let (expected_path, res) = match expectation {
        Some(Expectation::Get { path, res }) => (path, res),
        Some(other) => panic!("expected GET request but got {other:?}"),
        None => panic!("got another GET request, but didn't expect any more"),
    };

    let full_path = full_path(path, query);
    assert_eq!(full_path, expected_path, "GET request has unexpected path");
    res
}

fn post(
    state: State,
    path: Option<String>,
    query: Option<String>,
    req: serde_json::Value,
) -> serde_json::Value {
    let expectation = state.0.lock().unwrap().pop();
    let (expected_path, expected_req, res) = match expectation {
        Some(Expectation::Post { path, req, res }) => (path, req, res),
        Some(other) => panic!("expected POST request but got {other:?}"),
        None => panic!("got another POST request, but didn't expect any more"),
    };

    let full_path = full_path(path, query);
    assert_eq!(full_path, expected_path, "POST request has unexpected path");
    assert_eq!(req, expected_req, "POST request has unexpected body");
    res
}

fn full_path(path: Option<String>, query: Option<String>) -> String {
    let path = path.unwrap_or_default();
    match query {
        Some(query) => format!("{path}?{query}"),
        None => path,
    }
}
