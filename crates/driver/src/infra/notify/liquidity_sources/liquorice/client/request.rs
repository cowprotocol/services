pub use error::Error;
use {
    crate::infra::notify::liquidity_sources::liquorice::client::request::error::StatusError,
    reqwest::StatusCode,
    serde::de::DeserializeOwned,
    url::Url,
};

#[async_trait::async_trait]
pub trait IsRequest {
    type Response;
    async fn send(self, client: &reqwest::Client, base_url: &Url) -> Result<Self::Response, Error>;
}

pub mod error {
    #[derive(Debug, thiserror::Error)]
    pub enum Error {
        #[error("reqwest error: {0}")]
        Reqwest(#[from] reqwest::Error),
        #[error("status error: {0}")]
        Status(StatusError),
        #[error("unexpected error: {0}")]
        Unexpected(#[from] anyhow::Error),
    }
    #[derive(Debug, thiserror::Error)]
    #[error("{code}: {text}")]
    pub struct StatusError {
        pub code: reqwest::StatusCode,
        pub text: String,
    }
}

pub mod v1 {
    pub mod intent_origin {
        pub mod notification {
            pub mod post {
                use {
                    crate::infra::notify::liquidity_sources::liquorice::client::request::{
                        self,
                        Error,
                        decode_response,
                    },
                    anyhow::Context,
                    chrono::{DateTime, Utc, serde::ts_milliseconds},
                    serde::{Deserialize, Serialize},
                    std::collections::HashSet,
                    url::Url,
                };

                #[derive(Debug, Clone, Serialize, Deserialize)]
                #[serde(rename_all = "camelCase")]
                pub struct Response {}

                #[derive(Debug, Clone, Serialize, Deserialize)]
                #[serde(rename_all = "camelCase")]
                pub struct Request {
                    pub source: String,
                    pub metadata: Metadata,
                    #[serde(flatten)]
                    pub content: Content,
                    #[serde(with = "ts_milliseconds")]
                    pub timestamp: DateTime<Utc>,
                }

                #[derive(Debug, Clone, Serialize, Deserialize)]
                #[serde(rename_all = "camelCase")]
                pub struct Metadata {
                    pub driver_version: String,
                }

                #[derive(Debug, Clone, Serialize, Deserialize)]
                #[serde(tag = "type", content = "content", rename_all = "snake_case")]
                pub enum Content {
                    Settle(Settle),
                }

                #[derive(Debug, Clone, Serialize, Deserialize)]
                #[serde(rename_all = "camelCase")]
                pub struct Settle {
                    pub auction_id: i64,
                    pub rfq_ids: HashSet<String>,
                }

                #[async_trait::async_trait]
                impl request::IsRequest for Request {
                    type Response = Response;

                    async fn send(
                        self,
                        client: &reqwest::Client,
                        base_url: &Url,
                    ) -> Result<Self::Response, Error> {
                        let url = base_url
                            .to_owned()
                            .join("v1/intent-origin/notification")
                            .context("Parsing URL failed")?;

                        let response = client.post(url).json(&self).send().await?;
                        decode_response(response).await
                    }
                }
            }
        }
    }
}

async fn decode_response<T: DeserializeOwned>(response: reqwest::Response) -> Result<T, Error> {
    match response.status() {
        StatusCode::OK => response.json().await.map_err(Into::into),
        code => Err(Error::Status(StatusError {
            code,
            text: response.text().await?,
        })),
    }
}
