use crate::{
    domain,
    infra::{self, persistence::auction::dto},
};

impl infra::Orderbook {
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
