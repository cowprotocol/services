pub use error::Error;
use {
    crate::infra::notify::liquidity_sources::liquorice::client::request::error::StatusError,
    reqwest::StatusCode,
    serde::de::DeserializeOwned,
};

#[async_trait::async_trait]
pub trait Request {
    type Response;
    async fn send(self, client: &reqwest::Client, base_url: &str) -> Result<Self::Response, Error>;
}

pub mod error {
    #[derive(Debug, thiserror::Error)]
    pub enum Error {
        #[error("Reqwest error: {0}")]
        Reqwest(#[from] reqwest::Error),
        #[error("Status error")]
        Status(StatusError),
    }
    #[derive(Debug)]
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
                    chrono::{DateTime, Utc, serde::ts_milliseconds},
                    serde::{Deserialize, Serialize},
                    std::collections::HashSet,
                };

                pub type Response = ();

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

                // #[serde_as]
                #[derive(Debug, Clone, Serialize, Deserialize)]
                #[serde(tag = "type", content = "content", rename_all = "snake_case")]
                pub enum Content {
                    Settle(Settle),
                }

                // #[serde_as]
                #[derive(Debug, Clone, Serialize, Deserialize)]
                #[serde(rename_all = "camelCase")]
                pub struct Settle {
                    pub auction_id: i64,
                    pub rfq_ids: HashSet<String>,
                }

                #[async_trait::async_trait]
                impl request::Request for Request {
                    type Response = Response;

                    async fn send(
                        self,
                        client: &reqwest::Client,
                        base_url: &str,
                    ) -> Result<Self::Response, Error> {
                        let url = format!("{}/{}", base_url, "intent-origin/notification");
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
