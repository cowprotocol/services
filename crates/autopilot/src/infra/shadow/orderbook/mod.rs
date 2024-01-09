//! A client to the CoW Protocol public API.

use {
    crate::{domain, infra::persistence::auction::dto},
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
    pub async fn auction(&self) -> reqwest::Result<domain::AuctionWithId> {
        self.client
            .get(shared::url::join(&self.url, "api/v1/auction"))
            .send()
            .await?
            .error_for_status()?
            .json::<dto::AuctionWithId>()
            .await
            .map(Into::into)
    }
}
