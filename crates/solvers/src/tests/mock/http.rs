use {
    std::{
        fmt::{self, Debug, Formatter},
        net::SocketAddr,
        sync::{
            atomic::{AtomicBool, Ordering},
            Arc,
            Mutex,
        },
    },
    tokio::task::JoinHandle,
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

pub fn abort_on_panic() {
    let previous_hook = std::panic::take_hook();
    let new_hook = move |info: &std::panic::PanicInfo| {
        previous_hook(info);
        std::process::exit(1);
    };
    std::panic::set_hook(Box::new(new_hook));
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

/// Drop handle that will verify that the server task didn't panic throughout
/// the test and that all the expectations have been met.
pub struct ServerHandle {
    /// The address that handles requests to this server.
    pub address: SocketAddr,
    /// Handle to shut down the server task on drop.
    handle: JoinHandle<()>,
    /// Expectations that are left over after the test.
    expectations: Arc<Mutex<Vec<Expectation>>>,
    /// Indicates if some assertion failed.
    assert_failed: Arc<AtomicBool>,
}

impl Drop for ServerHandle {
    fn drop(&mut self) {
        // Don't cause mass hysteria!
        if std::thread::panicking() {
            return;
        }

        let server_panicked = self.assert_failed.load(std::sync::atomic::Ordering::SeqCst);
        // Panics happening in the server task might not cause the test to fail and only
        // show up if some assertion fails in the main task. This accomplishes that.
        assert!(!server_panicked);

        assert!(
            !self.handle.is_finished(),
            "mock http server terminated before test ended"
        );
        assert_eq!(self.expectations.lock().unwrap().len(), 0);
        self.handle.abort();
    }
}

/// Set up an mock external HTTP API.
pub async fn setup(mut expectations: Vec<Expectation>) -> ServerHandle {
    // Reverse expectations so test can specify them in natural order while allowing
    // us to simply `.pop()` the last element.
    expectations.reverse();

    let expectations = Arc::new(Mutex::new(expectations));
    let failed_assert = Arc::new(AtomicBool::new(false));

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
        .with_state(State {
            expectations: expectations.clone(),
            failed_assert: failed_assert.clone(),
        });

    let server = axum::Server::bind(&"0.0.0.0:0".parse().unwrap()).serve(app.into_make_service());
    let address = server.local_addr();
    let handle = tokio::spawn(async move { server.await.unwrap() });

    ServerHandle {
        handle,
        expectations,
        address,
        assert_failed: failed_assert,
    }
}

#[derive(Clone)]
struct State {
    /// Endpoint handler reads from here which request to expect and what to
    /// respond.
    expectations: Arc<Mutex<Vec<Expectation>>>,
    /// Request handler notifies test about failed assert via this mutex.
    failed_assert: Arc<AtomicBool>,
}

/// Runs the given closure and updates a flag if it panics.
fn assert_and_propagate_panics<F, R>(assertions: F, flag: &AtomicBool) -> R
where
    F: FnOnce() -> R + std::panic::UnwindSafe + 'static,
{
    std::panic::catch_unwind(assertions)
        .map_err(|_| {
            flag.store(true, Ordering::SeqCst);
        })
        .expect("ignore this panic; it was caused by the previous panic")
}

fn get(state: State, path: Option<String>, query: Option<String>) -> serde_json::Value {
    let expectation = state.expectations.lock().unwrap().pop();
    let assertions = || {
        let (expected_path, res) = match expectation {
            Some(Expectation::Get { path, res }) => (path, res),
            Some(other) => panic!("expected GET request but got {other:?}"),
            None => panic!("got another GET request, but didn't expect any more"),
        };

        let full_path = full_path(path, query);
        assert_eq!(full_path, expected_path, "GET request has unexpected path");
        res
    };
    assert_and_propagate_panics(assertions, &state.failed_assert)
}

fn post(
    state: State,
    path: Option<String>,
    query: Option<String>,
    req: serde_json::Value,
) -> serde_json::Value {
    let expectation = state.expectations.lock().unwrap().pop();

    let assertions = move || {
        let (expected_path, expected_req, res) = match expectation {
            Some(Expectation::Post { path, req, res }) => (path, req, res),
            Some(other) => panic!("expected POST request but got {other:?}"),
            None => panic!("got another POST request, but didn't expect any more"),
        };

        let full_path = full_path(path, query);
        assert_eq!(full_path, expected_path, "POST request has unexpected path");
        assert_eq!(req, expected_req, "POST request has unexpected body");
        res
    };

    assert_and_propagate_panics(assertions, &state.failed_assert)
}

fn full_path(path: Option<String>, query: Option<String>) -> String {
    let path = path.unwrap_or_default();
    match query {
        Some(query) => format!("{path}?{query}"),
        None => path,
    }
}
