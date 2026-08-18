//! HTTP client for the Solana driver.

#![expect(dead_code, reason = "consumed by the auction loop wiring")]

pub mod dto;

use {
    reqwest::StatusCode,
    serde::{Serialize, de::DeserializeOwned},
    std::time::Duration,
    url::Url,
};

/// Ceiling on one driver request. `/solve` covers the driver's own solver
/// round trips, so it is generous.
const REQUEST_TIMEOUT: Duration = Duration::from_secs(30);

/// A configured driver endpoint.
pub struct Driver {
    /// Name for logs and metrics.
    pub name: String,
    solve_url: Url,
    settle_url: Url,
    client: reqwest::Client,
}

/// A driver call that did not produce a usable response.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// The request never completed (connect, timeout, transport).
    #[error("transport: {0}")]
    Transport(#[from] reqwest::Error),
    /// The driver answered with a non-success status.
    #[error("status {status}: {body}")]
    Status { status: StatusCode, body: String },
    /// The response body did not match the expected DTO. Carries the body,
    /// the payload is the evidence.
    #[error("body: {error}: {body}")]
    Body {
        error: serde_json::Error,
        body: String,
    },
}

/// Append a path segment to the base URL. `Url::join` is RFC 3986 relative
/// resolution, which drops the base's last path segment unless it ends in a
/// slash, so a base like `http://driver/svm` would lose its prefix.
fn join(base: &Url, path: &str) -> Url {
    let base = base.as_str().trim_end_matches('/');
    let path = path.trim_start_matches('/');
    Url::parse(&format!("{base}/{path}")).expect("valid driver url")
}

impl Driver {
    pub fn new(name: String, url: &Url) -> Self {
        Self {
            name,
            solve_url: join(url, "solve"),
            settle_url: join(url, "settle"),
            client: reqwest::Client::builder()
                .timeout(REQUEST_TIMEOUT)
                .build()
                .expect("reqwest client"),
        }
    }

    /// Ask the driver for solutions to an auction.
    pub async fn solve(&self, request: &dto::SolveRequest) -> Result<dto::SolveResponse, Error> {
        self.post(self.solve_url.clone(), request).await
    }

    /// Ask the driver to submit a previously proposed solution.
    pub async fn settle(&self, request: &dto::SettleRequest) -> Result<dto::SettleResponse, Error> {
        self.post(self.settle_url.clone(), request).await
    }

    async fn post<Request, Response>(&self, url: Url, body: &Request) -> Result<Response, Error>
    where
        Request: Serialize,
        Response: DeserializeOwned,
    {
        let response = self.client.post(url).json(body).send().await?;
        let status = response.status();
        let body = response.text().await?;
        if !status.is_success() {
            return Err(Error::Status { status, body });
        }
        serde_json::from_str(&body).map_err(|error| Error::Body { error, body })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A base URL with a path and no trailing slash keeps its prefix,
    /// the case `Url::join` gets wrong.
    #[test]
    fn join_keeps_the_base_path() {
        let cases = [
            ("http://driver", "http://driver/solve"),
            ("http://driver/", "http://driver/solve"),
            ("http://driver/svm", "http://driver/svm/solve"),
            ("http://driver/svm/", "http://driver/svm/solve"),
        ];
        for (base, expected) in cases {
            let base = Url::parse(base).unwrap();
            assert_eq!(join(&base, "solve").as_str(), expected);
            assert_eq!(join(&base, "/solve").as_str(), expected);
        }
    }
}
