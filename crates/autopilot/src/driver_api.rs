use crate::driver_model::{execute, solve};
use anyhow::{anyhow, Context, Result};
use reqwest::Client;
use shared::http_client::response_body_with_size_limit;
use url::Url;

const RESPONSE_SIZE_LIMIT: usize = 10_000_000;

pub struct Driver {
    pub url: Url,
    pub client: Client,
}

impl Driver {
    pub async fn solve(&self, request: &solve::Request) -> Result<solve::Response> {
        self.request_response("solve", request).await
    }

    pub async fn execute(&self, request: &execute::Request) -> Result<execute::Response> {
        self.request_response("execute", request).await
    }

    async fn request_response<Response>(
        &self,
        route: &str,
        request: &impl serde::Serialize,
    ) -> Result<Response>
    where
        Response: serde::de::DeserializeOwned,
    {
        let mut url = self.url.clone();
        url.set_path(route);
        let request = self.client.post(url).json(request);
        let mut response = request.send().await.context("send")?;
        let status = response.status().as_u16();
        let body = response_body_with_size_limit(&mut response, RESPONSE_SIZE_LIMIT)
            .await
            .context("body")?;
        if status != 200 {
            let body = std::str::from_utf8(&body).context("body text")?;
            return Err(anyhow!("bad status {}, body {:?}", status, body));
        }
        serde_json::from_slice(&body).context("body json")
    }
}
