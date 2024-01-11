use {
    crate::{boundary, util},
    anyhow::{anyhow, Context, Result},
    reqwest::Client,
    std::time::Duration,
    url::Url,
};

const RESPONSE_SIZE_LIMIT: usize = 10_000_000;
const RESPONSE_TIME_LIMIT: Duration = Duration::from_secs(60);

pub struct Driver {
    pub name: String,
    pub url: Url,
    client: Client,
}

impl Driver {
    pub fn new(url: Url, name: String) -> Self {
        Self {
            name,
            url,
            client: Client::builder()
                .timeout(RESPONSE_TIME_LIMIT)
                .build()
                .unwrap(),
        }
    }

    pub async fn solve(
        &self,
        request: &boundary::solve::Request,
    ) -> Result<boundary::solve::Response> {
        self.request_response("solve", request, None).await
    }

    pub async fn reveal(
        &self,
        request: &boundary::reveal::Request,
    ) -> Result<boundary::reveal::Response> {
        self.request_response("reveal", request, None).await
    }

    pub async fn settle(
        &self,
        request: &boundary::settle::Request,
        timeout: std::time::Duration,
    ) -> Result<boundary::settle::Response> {
        self.request_response("settle", request, Some(timeout))
            .await
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
