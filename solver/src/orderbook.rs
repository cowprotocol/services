use model::order::Order;
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

    pub async fn get_orders(&self) -> reqwest::Result<Vec<Order>> {
        const PATH: &str = "/api/v1/solvable_orders";
        let mut url = self.base.clone();
        url.set_path(PATH);
        self.client.get(url).send().await?.json().await
    }
}

#[cfg(test)]
pub mod test_util {
    use super::*;

    // cargo test real_orderbook -- --ignored --nocapture
    #[tokio::test]
    #[ignore]
    async fn real_orderbook() {
        let api = OrderBookApi::new(Url::parse("http://localhost:8080").unwrap(), Client::new());
        let orders = api.get_orders().await.unwrap();
        println!("{:?}", orders);
    }
}
