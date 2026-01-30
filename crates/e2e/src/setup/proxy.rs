//! Simple HTTP reverse proxy with automatic failover for e2e testing.
//!
//! This module provides a test-only reverse proxy that simulates how Kubernetes
//! service pools work in production. In production, when multiple instances of
//! a service run (e.g., autopilot with leader/follower pattern), Kubernetes
//! routes traffic to the active instance and automatically fails over to a
//! backup when the primary becomes unavailable.
//!
//! The proxy maintains a queue of backend URLs and automatically rotates
//! through them when the currently active backend fails. This allows e2e tests
//! to simulate production failover behavior without requiring a full k8s
//! cluster.

use {
    axum::{Router, body::Body, http::Request, response::IntoResponse},
    hyper::body::to_bytes,
    std::{collections::VecDeque, net::SocketAddr, sync::Arc},
    tokio::{sync::RwLock, task::JoinHandle},
    url::Url,
};

/// HTTP reverse proxy with automatic failover that permanently switches
/// to the fallback backend when the current backend fails.
///
/// This simulates k8s service pools where traffic is automatically routed
/// to healthy backend instances.
pub struct ReverseProxy {
    _server_handle: JoinHandle<()>,
}

#[derive(Clone)]
struct ProxyState {
    backends: Arc<RwLock<VecDeque<Url>>>,
}

impl ProxyState {
    /// Returns the current active backend URL.
    async fn get_current_backend(&self) -> Url {
        self.backends
            .read()
            .await
            .front()
            .cloned()
            .expect("backends should never be empty")
    }

    /// Rotates to the next backend by moving the current backend to the end of
    /// the queue.
    async fn rotate_backends(&self) {
        let mut backends = self.backends.write().await;
        if let Some(current) = backends.pop_front() {
            backends.push_back(current);
        }
        tracing::info!(backends = ?backends.iter().map(Url::as_str).collect::<Vec<_>>(), "rotated backends");
    }

    /// Returns the total number of backends configured.
    ///
    /// Used to determine how many retry attempts to make before giving up.
    async fn backend_count(&self) -> usize {
        self.backends.read().await.len()
    }
}

impl ReverseProxy {
    /// Start a new proxy server with automatic failover between backends
    ///
    /// # Panics
    /// Panics if `backends` is empty. At least one backend URL is required.
    pub fn start(listen_addr: SocketAddr, backends: &[Url]) -> Self {
        assert!(
            !backends.is_empty(),
            "At least one backend URL is required for the proxy"
        );

        let backends_queue: VecDeque<Url> = backends.iter().cloned().collect();

        let state = ProxyState {
            backends: Arc::new(RwLock::new(backends_queue)),
        };

        let backends_log: Vec<Url> = backends.to_vec();
        let server_handle = tokio::spawn(serve(listen_addr, backends_log, state));

        Self {
            _server_handle: server_handle,
        }
    }
}

async fn serve(listen_addr: SocketAddr, backends: Vec<Url>, state: ProxyState) {
    let client = reqwest::Client::new();

    let proxy_handler = move |req: Request<Body>| {
        let client = client.clone();
        let state = state.clone();
        async move { handle_request(client, state, req).await }
    };

    let app = Router::new().fallback(proxy_handler);

    tracing::info!(%listen_addr, backends = ?backends.iter().map(Url::as_str).collect::<Vec<_>>(), "starting reverse proxy");
    axum::Server::bind(&listen_addr)
        .serve(app.into_make_service())
        .await
        .unwrap();
}

async fn handle_request(
    client: reqwest::Client,
    state: ProxyState,
    req: Request<Body>,
) -> impl IntoResponse {
    let (parts, body) = req.into_parts();

    // Convert body to bytes once for reuse across retries
    let body_bytes = match to_bytes(body).await {
        Ok(bytes) => bytes,
        Err(err) => {
            return (
                axum::http::StatusCode::BAD_REQUEST,
                format!("Failed to read request body: {}", err),
            )
                .into_response();
        }
    };

    let backend_count = state.backend_count().await;

    for attempt in 0..backend_count {
        let backend = state.get_current_backend().await;

        match try_backend(&client, &parts, body_bytes.to_vec(), &backend).await {
            Ok(response) => return response.into_response(),
            Err(err) => {
                tracing::warn!(?err, %backend, attempt, "backend failed, rotating to next");
                state.rotate_backends().await;
            }
        }
    }

    (
        axum::http::StatusCode::BAD_GATEWAY,
        "All backends unavailable",
    )
        .into_response()
}

async fn try_backend(
    client: &reqwest::Client,
    parts: &axum::http::request::Parts,
    body: Vec<u8>,
    backend: &Url,
) -> Result<(axum::http::StatusCode, Vec<u8>), reqwest::Error> {
    let path = parts
        .uri
        .path_and_query()
        .map(|pq| pq.as_str())
        .unwrap_or("");

    // Build the full URL by combining backend and path
    let url = format!("{}{}", backend, path);
    // Build a reqwest request with the same method
    let mut backend_req = client.request(parts.method.clone(), &url);
    // Forward all headers from the original request
    for (name, value) in &parts.headers {
        backend_req = backend_req.header(name, value);
    }

    // Attach the body
    backend_req = backend_req.body(body);

    let backend_resp = backend_req.send().await?;
    let status = axum::http::StatusCode::from_u16(backend_resp.status().as_u16()).unwrap();
    let bytes = backend_resp.bytes().await?;
    Ok((status, bytes.to_vec()))
}
