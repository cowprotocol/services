use {
    anyhow::Result,
    model::auction::AuctionWithId,
    reqwest::{Client, Url},
};

pub struct OrderBookApi {
    base: Url,
    client: Client,
    competition_auth: Option<String>,
}

impl OrderBookApi {
    /// base: protocol and host of the url. example: `https://example.com`
    pub fn new(base: Url, client: Client, competition_auth: Option<String>) -> Self {
        Self {
            base,
            client,
            competition_auth,
        }
    }

    pub async fn get_auction(&self) -> Result<AuctionWithId> {
        let url = shared::url::join(&self.base, "api/v1/auction");
        let response = self.client.get(url).send().await?;
        if let Err(err) = response.error_for_status_ref() {
            let body = response.text().await;
            return Err(anyhow::Error::new(err).context(format!("body: {body:?}")));
        }
        let auction = response.json().await?;
        Ok(auction)
    }

    /// If this is false then sending solver competition most likely fails.
    pub fn is_authenticated(&self) -> bool {
        self.competition_auth.is_some()
    }
}

#[cfg(test)]
pub mod test_util {
    use super::*;

    // cargo test local_orderbook -- --ignored --nocapture
    #[tokio::test]
    #[ignore]
    async fn local_orderbook() {
        let api = OrderBookApi::new(
            Url::parse("http://localhost:8080").unwrap(),
            Client::new(),
            None,
        );
        let auction = api.get_auction().await.unwrap();
        println!("{auction:#?}");
    }

    // cargo test real_orderbook -- --ignored --nocapture
    #[tokio::test]
    #[ignore]
    async fn real_orderbook() {
        let api = OrderBookApi::new(
            Url::parse("https://barn.api.cow.fi/mainnet/").unwrap(),
            Client::new(),
            None,
        );
        let auction = api.get_auction().await.unwrap();
        println!("{auction:#?}");
    }
}
