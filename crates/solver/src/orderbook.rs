use anyhow::Result;
use model::auction::Auction;
use reqwest::{Client, Url};

pub struct OrderBookApi {
    base: Url,
    client: Client,
}

impl OrderBookApi {
    /// base: protocol and host of the url. example: `https://example.com`
    pub fn new(base: Url, client: Client) -> Self {
        Self { base, client }
    }

    pub async fn get_auction(&self) -> Result<Auction> {
        let url = self.base.join("api/v1/auction")?;
        let auction = self.client.get(url).send().await?.json().await?;
        Ok(auction)
    }
}

#[cfg(test)]
pub mod test_util {
    use super::*;

    // cargo test local_orderbook -- --ignored --nocapture
    #[tokio::test]
    #[ignore]
    async fn local_orderbook() {
        let api = OrderBookApi::new(Url::parse("http://localhost:8080").unwrap(), Client::new());
        let auction = api.get_auction().await.unwrap();
        println!("{:#?}", auction);
    }

    // cargo test real_orderbook -- --ignored --nocapture
    #[tokio::test]
    #[ignore]
    async fn real_orderbook() {
        let api = OrderBookApi::new(
            Url::parse("https://barn.api.cow.fi/mainnet/").unwrap(),
            Client::new(),
        );
        let auction = api.get_auction().await.unwrap();
        println!("{:#?}", auction);
    }
}
