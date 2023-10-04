use {
    crate::driver_model::{reveal, settle, solve},
    anyhow::{anyhow, Context, Result},
    reqwest::Client,
    shared::http_client::response_body_with_size_limit,
    std::time::Duration,
    url::Url,
};

const RESPONSE_SIZE_LIMIT: usize = 10_000_000;
const RESPONSE_TIME_LIMIT: Duration = Duration::from_secs(60);

pub struct Driver {
    pub url: Url,
    client: Client,
}

impl Driver {
    pub fn new(url: Url) -> Self {
        Self {
            url,
            client: Client::builder()
                .timeout(RESPONSE_TIME_LIMIT)
                .build()
                .unwrap(),
        }
    }

    pub async fn solve(&self, request: &solve::Request) -> Result<solve::Response> {
        self.request_response("solve", Some(request)).await
    }

    pub async fn reveal(&self, request: &reveal::Request) -> Result<reveal::Response> {
        self.request_response("reveal", Some(request)).await
    }

    pub async fn settle(&self, request: &settle::Request) -> Result<settle::Response> {
        self.request_response("settle", Some(request)).await
    }

    async fn request_response<Response>(
        &self,
        path: &str,
        request: Option<&impl serde::Serialize>,
    ) -> Result<Response>
    where
        Response: serde::de::DeserializeOwned,
    {
        let url = shared::url::join(&self.url, path);
        let request = if let Some(request) = request {
            tracing::trace!(
                path=&url.path(),
                body=%serde_json::to_string_pretty(request).unwrap(),
                "request",
            );
            self.client.post(url.clone()).json(request)
        } else {
            tracing::trace!(path=%url.path(), "request");
            self.client.post(url.clone())
        };
        let mut response = request.send().await.context("send")?;
        let status = response.status().as_u16();
        let body = response_body_with_size_limit(&mut response, RESPONSE_SIZE_LIMIT)
            .await
            .context("body")?;
        let text = String::from_utf8_lossy(&body);
        tracing::trace!(%status, body=%text, "response");
        let context = || format!("url {url}, body {text:?}");
        if status != 200 {
            return Err(anyhow!("bad status {status}, {}", context()));
        }
        serde_json::from_slice(&body).with_context(|| format!("bad json {}", context()))
    }
}
