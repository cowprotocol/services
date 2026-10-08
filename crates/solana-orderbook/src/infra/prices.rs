//! Client for the autopilot's native price route, the orderbook's only price
//! source.

use {
    anyhow::{Context, anyhow},
    reqwest::StatusCode,
    serde::Deserialize,
    serde_with::{DisplayFromStr, serde_as},
    solana_sdk::pubkey::Pubkey,
    std::time::Duration,
    url::Url,
};

/// Reads native prices from the autopilot, which keeps the cache the auctions
/// are scored with.
#[derive(Clone, Debug)]
pub struct NativePrices {
    client: reqwest::Client,
    autopilot: Url,
    timeout: Duration,
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// No estimator prices the token.
    #[error("no native price")]
    NoLiquidity,
    /// The autopilot's price sources are rate limited.
    #[error("native price lookup rate limited")]
    RateLimited,
    #[error(transparent)]
    Internal(#[from] anyhow::Error),
}

/// The `GET /native_price/{mint}` body.
#[serde_as]
#[derive(Debug, Deserialize)]
struct Response {
    #[serde_as(as = "DisplayFromStr")]
    price: u64,
}

impl NativePrices {
    pub fn new(autopilot: Url, timeout: Duration) -> Self {
        Self {
            client: reqwest::Client::new(),
            autopilot,
            timeout,
        }
    }

    /// Lamports per 10^9 atoms of `token`, the unit the autopilot prices
    /// auctions in.
    pub async fn price(&self, token: Pubkey) -> Result<u64, Error> {
        // `Url::join` resolves relative to the last slash and would drop a
        // final path segment of a base configured without a trailing slash.
        let url = format!(
            "{}/native_price/{token}?timeout_ms={}",
            self.autopilot.as_str().trim_end_matches('/'),
            self.timeout.as_millis()
        );
        let mut request = self.client.get(url).timeout(self.timeout);
        if let Some(id) = observe::tracing::distributed::request_id::from_current_span() {
            request = request.header("X-REQUEST-ID", id);
        }
        let response = request.send().await.context("native price request")?;
        match response.status() {
            StatusCode::OK => {
                let body: Response = response.json().await.context("native price response")?;
                Ok(body.price)
            }
            StatusCode::NOT_FOUND => Err(Error::NoLiquidity),
            StatusCode::TOO_MANY_REQUESTS => Err(Error::RateLimited),
            status => {
                let body = response.text().await.unwrap_or_default();
                Err(anyhow!("autopilot answered {status}: {body}").into())
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// An autopilot stand-in answering every mint under `path` with `status`
    /// and `body`.
    async fn autopilot(path: &'static str, status: StatusCode, body: &'static str) -> Url {
        let app = axum::Router::new().route(
            path,
            axum::routing::get(move || async move { (status, body) }),
        );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        format!("http://{addr}").parse().unwrap()
    }

    async fn price(autopilot: Url) -> Result<u64, Error> {
        NativePrices::new(autopilot, Duration::from_secs(1))
            .price(Pubkey::new_unique())
            .await
    }

    /// The EVM forwarder's mapping: 404 is no liquidity, 429 rate limited,
    /// and anything else, a dead autopilot included, is internal.
    #[tokio::test]
    async fn maps_the_autopilot_answers() {
        let route = "/native_price/{mint}";
        let priced = autopilot(route, StatusCode::OK, r#"{"price":"5000000000"}"#).await;
        assert_eq!(price(priced).await.unwrap(), 5_000_000_000);
        let unpriced = autopilot(route, StatusCode::NOT_FOUND, "No liquidity").await;
        assert!(matches!(price(unpriced).await, Err(Error::NoLiquidity)));
        let limited = autopilot(route, StatusCode::TOO_MANY_REQUESTS, "Rate limited").await;
        assert!(matches!(price(limited).await, Err(Error::RateLimited)));
        let broken = autopilot(route, StatusCode::INTERNAL_SERVER_ERROR, "Internal error").await;
        assert!(matches!(price(broken).await, Err(Error::Internal(_))));
        let dead = "http://127.0.0.1:1".parse().unwrap();
        assert!(matches!(price(dead).await, Err(Error::Internal(_))));
    }

    /// The stand-in echoes `timeout_ms` as the price.
    #[tokio::test]
    async fn sends_the_timeout() {
        let app = axum::Router::new().route(
            "/native_price/{mint}",
            axum::routing::get(
                |axum::extract::Query(query): axum::extract::Query<
                    std::collections::HashMap<String, String>,
                >| async move { format!(r#"{{"price":"{}"}}"#, query["timeout_ms"]) },
            ),
        );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });

        let autopilot = format!("http://{addr}").parse().unwrap();
        assert_eq!(price(autopilot).await.unwrap(), 1000);
    }

    /// A base URL with a path keeps it, with or without a trailing slash.
    #[tokio::test]
    async fn keeps_the_base_path() {
        let base = autopilot(
            "/autopilot/native_price/{mint}",
            StatusCode::OK,
            r#"{"price":"1"}"#,
        )
        .await;
        for path in ["autopilot", "autopilot/"] {
            let autopilot = base.join(path).unwrap();
            assert_eq!(price(autopilot).await.unwrap(), 1, "{path}");
        }
    }
}
