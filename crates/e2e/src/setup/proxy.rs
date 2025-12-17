use {
    axum::{Router, body::Body, http::Request, response::IntoResponse},
    std::{net::SocketAddr, sync::Arc},
    tokio::{sync::RwLock, task::JoinHandle},
    url::Url,
};

/// HTTP reverse proxy with automatic failover that permanently switches
/// to the fallback backend when the current backend fails.
pub struct NativePriceProxy {
    _server_handle: JoinHandle<()>,
}

#[derive(Clone)]
struct ProxyState {
    primary: Url,
    secondary: Url,
    active: Arc<RwLock<ActiveBackend>>,
}

#[derive(Clone, Copy, PartialEq, Debug)]
enum ActiveBackend {
    Primary,
    Secondary,
}

impl NativePriceProxy {
    /// Start a new proxy server with automatic failover between backends
    pub fn start(listen_addr: SocketAddr, primary: Url, secondary: Url) -> Self {
        let state = ProxyState {
            primary: primary.clone(),
            secondary: secondary.clone(),
            active: Arc::new(RwLock::new(ActiveBackend::Primary)),
        };

        let server_handle = tokio::spawn(async move {
            let client = reqwest::Client::new();

            let proxy_handler = move |req: Request<Body>| {
                let client = client.clone();
                let state = state.clone();
                async move {
                    let path = req
                        .uri()
                        .path_and_query()
                        .map(|pq: &axum::http::uri::PathAndQuery| pq.as_str())
                        .unwrap_or("");

                    // Try current active backend
                    let active = *state.active.read().await;
                    let (current_url, fallback) = match active {
                        ActiveBackend::Primary => (
                            format!("{}{}", state.primary, path),
                            ActiveBackend::Secondary,
                        ),
                        ActiveBackend::Secondary => (
                            format!("{}{}", state.secondary, path),
                            ActiveBackend::Primary,
                        ),
                    };

                    match try_backend(&client, &current_url).await {
                        Ok(response) => return response.into_response(),
                        Err(err) => {
                            tracing::warn!(
                                ?err,
                                ?active,
                                "active backend failed, switching to fallback"
                            );
                            // Switch to fallback
                            *state.active.write().await = fallback;
                        }
                    }

                    // Try fallback backend
                    let fallback_url = match fallback {
                        ActiveBackend::Primary => format!("{}{}", state.primary, path),
                        ActiveBackend::Secondary => format!("{}{}", state.secondary, path),
                    };

                    match try_backend(&client, &fallback_url).await {
                        Ok(response) => response.into_response(),
                        Err(_) => (
                            axum::http::StatusCode::BAD_GATEWAY,
                            "Both backends unavailable",
                        )
                            .into_response(),
                    }
                }
            };

            let app = Router::new().fallback(proxy_handler);

            tracing::info!(
                ?listen_addr,
                ?primary,
                ?secondary,
                "starting native price proxy"
            );
            axum::Server::bind(&listen_addr)
                .serve(app.into_make_service())
                .await
                .unwrap();
        });

        Self {
            _server_handle: server_handle,
        }
    }
}

async fn try_backend(
    client: &reqwest::Client,
    url: &str,
) -> Result<(axum::http::StatusCode, Vec<u8>), reqwest::Error> {
    let backend_resp = client.get(url).send().await?;
    let status = axum::http::StatusCode::from_u16(backend_resp.status().as_u16()).unwrap();
    let bytes = backend_resp.bytes().await?;
    Ok((status, bytes.to_vec()))
}
