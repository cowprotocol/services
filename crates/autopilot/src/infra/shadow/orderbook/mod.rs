//! A client to the CoW Protocol public API.

use {
    crate::{domain, infra::persistence::dto},
    reqwest::Url,
};

pub struct Orderbook {
    client: reqwest::Client,
    url: Url,
}

impl Orderbook {
    /// Creates a new CoW Protocol client.
    pub fn new(client: reqwest::Client, url: Url) -> Self {
        Self { client, url }
    }

    /// Retrieves the current auction.
    pub async fn auction(&self) -> anyhow::Result<domain::Auction> {
        self.client
            .get(shared::url::join(&self.url, "api/v1/auction"))
            .send()
            .await?
            .error_for_status()?
            .json::<dto::Auction>()
            .await
            .map(TryInto::try_into)
            .map_err(Into::<anyhow::Error>::into)?
    }
}
