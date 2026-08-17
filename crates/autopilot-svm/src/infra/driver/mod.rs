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
    /// The response body did not match the expected DTO.
    #[error("body: {0}")]
    Body(#[from] serde_json::Error),
}

impl Driver {
    pub fn new(name: String, url: &Url) -> Self {
        Self {
            name,
            solve_url: url.join("solve").expect("valid driver url"),
            settle_url: url.join("settle").expect("valid driver url"),
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
        Ok(serde_json::from_str(&body)?)
    }
}
