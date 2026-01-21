use {
    self::dto::{reveal, settle, solve},
    crate::{arguments::Account, domain::eth, infra::solvers::dto::notify, util},
    alloy::signers::{Signer, aws::AwsSigner},
    anyhow::{Context, Result, anyhow},
    chrono::{DateTime, Utc},
    observe::tracing::tracing_headers,
    reqwest::{Client, StatusCode},
    std::{sync::Arc, time::Duration},
    thiserror::Error,
    tracing::instrument,
    url::Url,
};

pub mod dto;

const RESPONSE_SIZE_LIMIT: usize = 10_000_000;
const RESPONSE_TIME_LIMIT: Duration = Duration::from_secs(60);

pub struct Driver {
    pub name: String,
    pub url: Url,
    // An optional threshold used to check "fairness" of provided solutions. If specified, a
    // winning solution should be discarded if it contains at least one order, which
    // another driver solved with surplus exceeding this driver's surplus by `threshold`
    pub fairness_threshold: Option<eth::Ether>,
    pub submission_address: eth::Address,
    pub requested_timeout_on_problems: bool,
    client: Client,
}

#[derive(Error, Debug)]
pub enum Error {
    #[error("unable to load KMS account")]
    UnableToLoadKmsAccount,
    #[error("failed to build client")]
    FailedToBuildClient(#[source] reqwest::Error),
}

impl Driver {
    #[instrument(skip_all)]
    pub async fn try_new(
        url: Url,
        name: String,
        fairness_threshold: Option<eth::Ether>,
        submission_account: Account,
        requested_timeout_on_problems: bool,
    ) -> Result<Self, Error> {
        let submission_address = match submission_account {
            Account::Kms(key_id) => {
                let config = alloy::signers::aws::aws_config::load_from_env().await;
                let client = alloy::signers::aws::aws_sdk_kms::Client::new(&config);
                let account = AwsSigner::new(client, key_id.0.clone(), None)
                    .await
                    .map_err(|_| {
                        tracing::error!(?name, ?key_id, "Unable to load KMS account");
                        Error::UnableToLoadKmsAccount
                    })?;
                account.address()
            }
            Account::Address(address) => address,
        };
        tracing::info!(
            ?name,
            ?url,
            ?fairness_threshold,
            ?submission_address,
            "Creating solver"
        );

        Ok(Self {
            name,
            url,
            fairness_threshold,
            client: Client::builder()
                .timeout(RESPONSE_TIME_LIMIT)
                .tcp_keepalive(Duration::from_secs(15))
                .build()
                .map_err(Error::FailedToBuildClient)?,
            submission_address,
            requested_timeout_on_problems,
        })
    }

    pub async fn solve(&self, request: solve::Request) -> Result<solve::Response> {
        self.request_response("solve", request).await
    }

    pub async fn reveal(&self, request: reveal::Request) -> Result<reveal::Response> {
        self.request_response("reveal", request).await
    }

    pub async fn settle(
        &self,
        request: &settle::Request,
        timeout: std::time::Duration,
    ) -> Result<()> {
        let url = util::join(&self.url, "settle");
        tracing::trace!(
            path=&url.path(),
            body=%serde_json::to_string_pretty(request).unwrap(),
            "solver request",
        );

        let response = self
            .client
            .post(url)
            .json(request)
            .timeout(timeout)
            .header("X-REQUEST-ID", request.auction_id.to_string())
            .headers(tracing_headers())
            .send()
            .await
            .context("send")?;
        let status = response.status();

        tracing::trace!(%status, "solver response");

        if status != StatusCode::OK {
            let text = response.text().await.context("read error response body")?;
            return Err(anyhow!("bad status {status}: {text}"));
        }
        Ok(())
    }

    pub async fn notify(&self, request: notify::Request) -> Result<()> {
        self.request_response("notify", request).await
    }

    async fn request_response<Response, Request>(
        &self,
        path: &str,
        request: Request,
    ) -> Result<Response>
    where
        Response: serde::de::DeserializeOwned,
        Request: serde::Serialize + Send + Sync + 'static,
    {
        let url = util::join(&self.url, path);
        tracing::trace!(
            path=&url.path(),
            body=%serde_json::to_string_pretty(&request).unwrap(),
            "solver request",
        );
        let mut request = {
            let builder = self.client.post(url.clone()).headers(tracing_headers());
            // If the payload is very big then serializing it will block the
            // executor a long time (mostly relevant for solve requests).
            // That's why we always do it on a thread specifically for
            // running blocking tasks.
            tokio::task::spawn_blocking(move || builder.json(&request))
                .await
                .context("failed to build request")?
        };

        if let Some(request_id) = observe::distributed_tracing::request_id::from_current_span() {
            request = request.header("X-REQUEST-ID", request_id);
        }

        let mut response = request.send().await.context("send")?;
        let status = response.status().as_u16();
        let body = response_body_with_size_limit(&mut response, RESPONSE_SIZE_LIMIT)
            .await
            .context("body")?;
        let text = String::from_utf8_lossy(&body);
        tracing::trace!(%status, body=%text, "solver response");
        let context = || format!("url {url}, body {text:?}");
        if status != 200 {
            return Err(anyhow!("bad status {status}, {}", context()));
        }
        serde_json::from_slice(&body).with_context(|| format!("bad json {}", context()))
    }
}

/// Extracts the bytes of the response up to some size limit.
///
/// Returns an error if the byte limit was exceeded.
pub async fn response_body_with_size_limit(
    response: &mut reqwest::Response,
    limit: usize,
) -> Result<Vec<u8>> {
    let mut bytes = Vec::new();
    while let Some(chunk) = response.chunk().await? {
        let slice: &[u8] = &chunk;
        if bytes.len() + slice.len() > limit {
            return Err(anyhow!("size limit exceeded"));
        }
        bytes.extend_from_slice(slice);
    }
    Ok(bytes)
}

/// Notifies the non-settling driver in a fire-and-forget manner.
pub fn notify_banned_solver(
    non_settling_driver: Arc<Driver>,
    reason: notify::BanReason,
    banned_until: DateTime<Utc>,
) {
    let request = notify::Request::Banned {
        reason,
        until: banned_until,
    };
    tokio::spawn(async move {
        let _ = non_settling_driver.notify(request).await;
    });
}
