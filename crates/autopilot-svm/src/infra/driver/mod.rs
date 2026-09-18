//! HTTP client for the Solana driver.

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

impl Error {
    /// Whether a failed `/settle` provably never sent the settlement
    /// transaction, so the orders can re-enter auctions immediately. Every
    /// listed kind rejects before the send. `DeadlineExceeded` stays out: the
    /// driver also answers it when the confirmation wait expired with the
    /// transaction already on the wire. Transport failures reveal nothing.
    pub fn settlement_provably_unsent(&self) -> bool {
        let Error::Status { body, .. } = self else {
            return false;
        };
        let kind = serde_json::from_str::<serde_json::Value>(body)
            .ok()
            .and_then(|body| {
                body.get("kind")
                    .and_then(|kind| kind.as_str().map(String::from))
            });
        matches!(
            kind.as_deref(),
            Some(
                "InvalidAuctionId"
                    | "SolutionNotAvailable"
                    | "TooManyPendingSettlements"
                    | "InvalidCreation"
                    | "FailedToCreate"
                    | "SimulationFailed"
            )
        )
    }
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

    #[test]
    fn only_pre_send_rejections_count_as_provably_unsent() {
        let status = |kind: &str| Error::Status {
            status: StatusCode::BAD_REQUEST,
            body: format!(r#"{{"kind":"{kind}","description":""}}"#),
        };
        assert!(status("SimulationFailed").settlement_provably_unsent());
        assert!(status("SolutionNotAvailable").settlement_provably_unsent());
        // The driver answers this both before and after the send.
        assert!(!status("DeadlineExceeded").settlement_provably_unsent());
        assert!(!status("FailedToSubmit").settlement_provably_unsent());
        let garbage = Error::Status {
            status: StatusCode::BAD_GATEWAY,
            body: "not json".to_string(),
        };
        assert!(!garbage.settlement_provably_unsent());
    }

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
