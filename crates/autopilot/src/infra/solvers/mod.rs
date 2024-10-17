use {
    self::dto::{reveal, settle, solve},
    crate::{domain::eth, util},
    anyhow::{anyhow, Context, Result},
    reqwest::{Client, StatusCode},
    std::time::Duration,
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
    client: Client,
}

impl Driver {
    pub fn new(
        url: Url,
        name: String,
        fairness_threshold: Option<eth::Ether>,
        submission_address: eth::Address,
    ) -> Self {
        Self {
            name,
            url,
            fairness_threshold,
            client: Client::builder()
                .timeout(RESPONSE_TIME_LIMIT)
                .build()
                .unwrap(),
            submission_address,
        }
    }

    pub async fn solve(&self, request: &solve::Request) -> Result<solve::Response> {
        self.request_response("solve", request, None).await
    }

    pub async fn reveal(&self, request: &reveal::Request) -> Result<reveal::Response> {
        self.request_response("reveal", request, None).await
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

    async fn request_response<Response>(
        &self,
        path: &str,
        request: &impl serde::Serialize,
        timeout: Option<std::time::Duration>,
    ) -> Result<Response>
    where
        Response: serde::de::DeserializeOwned,
    {
        let url = util::join(&self.url, path);
        tracing::trace!(
            path=&url.path(),
            body=%serde_json::to_string_pretty(request).unwrap(),
            "solver request",
        );
        let mut request = self.client.post(url.clone()).json(request);

        if let Some(timeout) = timeout {
            request = request.timeout(timeout);
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
