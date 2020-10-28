use model::Order;
use reqwest::{Client, Url};
use std::time::Duration;

struct OrderbookApi {
    base: Url,
    client: Client,
}

impl OrderbookApi {
    /// base: protocol and host of the url. example: `https://example.com`
    pub fn new(base: Url, request_timeout: Duration) -> Self {
        // Unwrap because we cannot handle client creation failing.
        let client = Client::builder().timeout(request_timeout).build().unwrap();
        Self { base, client }
    }

    pub async fn get_orders(&self) -> reqwest::Result<Vec<Order>> {
        const PATH: &str = "/api/v1/orders";
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
        let api = OrderbookApi::new(
            Url::parse("http://localhost:8080").unwrap(),
            Duration::from_secs(10),
        );
        let orders = api.get_orders().await.unwrap();
        println!("{:?}", orders);
    }
}
