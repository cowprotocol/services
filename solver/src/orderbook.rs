use contracts::WETH9;
use model::order::Order;
use reqwest::{Client, Url};
use std::time::Duration;

pub struct OrderBookApi {
    base: Url,
    client: Client,
    native_token: WETH9,
}

impl OrderBookApi {
    /// base: protocol and host of the url. example: `https://example.com`
    pub fn new(base: Url, request_timeout: Duration, native_token: WETH9) -> Self {
        // Unwrap because we cannot handle client creation failing.
        let client = Client::builder().timeout(request_timeout).build().unwrap();
        Self {
            base,
            client,
            native_token,
        }
    }

    pub async fn get_orders(&self) -> reqwest::Result<Vec<Order>> {
        const PATH: &str = "/api/v1/solvable_orders";
        let mut url = self.base.clone();
        url.set_path(PATH);
        self.client.get(url).send().await?.json().await
    }

    pub fn get_native_token(&self) -> WETH9 {
        self.native_token.clone()
    }
}

#[cfg(test)]
pub mod test_util {
    use super::*;
    use crate::testutil;
    use ethcontract::H160;

    // cargo test real_orderbook -- --ignored --nocapture
    #[tokio::test]
    #[ignore]
    async fn real_orderbook() {
        let native_token = testutil::dummy_weth(H160([0x42; 20]));
        let api = OrderBookApi::new(
            Url::parse("http://localhost:8080").unwrap(),
            Duration::from_secs(10),
            native_token,
        );
        let orders = api.get_orders().await.unwrap();
        println!("{:?}", orders);
    }
}
