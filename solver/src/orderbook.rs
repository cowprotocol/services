use contracts::WETH9;
use ethcontract::H160;
use model::order::Order;
use reqwest::{Client, Url};
use std::collections::HashSet;

pub struct OrderBookApi {
    base: Url,
    client: Client,
    native_token: WETH9,
    liquidity_order_owners: HashSet<H160>,
}

impl OrderBookApi {
    /// base: protocol and host of the url. example: `https://example.com`
    pub fn new(
        base: Url,
        native_token: WETH9,
        client: Client,
        liquidity_order_owners: HashSet<H160>,
    ) -> Self {
        Self {
            base,
            client,
            native_token,
            liquidity_order_owners,
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

    pub fn liquidity_order_owners(&self) -> &HashSet<H160> {
        &self.liquidity_order_owners
    }
}

#[cfg(test)]
pub mod test_util {
    use super::*;
    use shared::dummy_contract;

    // cargo test real_orderbook -- --ignored --nocapture
    #[tokio::test]
    #[ignore]
    async fn real_orderbook() {
        let native_token = dummy_contract!(WETH9, [0x42; 20]);
        let api = OrderBookApi::new(
            Url::parse("http://localhost:8080").unwrap(),
            native_token,
            Client::new(),
            Default::default(),
        );
        let orders = api.get_orders().await.unwrap();
        println!("{:?}", orders);
    }
}
