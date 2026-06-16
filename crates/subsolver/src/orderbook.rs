use {
    alloy_primitives::{Address, U256},
    number::serialization::HexOrDecimalU256,
    serde::Deserialize,
    serde_with::serde_as,
    std::collections::BTreeMap,
    url::Url,
};

pub struct OrderbookClient {
    http: reqwest::Client,
    base_url: Url,
}

impl OrderbookClient {
    pub fn new(base_url: Url) -> Self {
        Self {
            http: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(10))
                .build()
                .expect("failed to build HTTP client"),
            base_url,
        }
    }

    pub async fn get_auction(&self) -> anyhow::Result<AuctionResponse> {
        let url = self
            .base_url
            .join("/api/v1/auction")
            .expect("valid url join");
        let response = self.http.get(url).send().await?;
        let body = response.error_for_status()?.json().await?;
        Ok(body)
    }
}

#[serde_as]
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AuctionResponse {
    pub id: i64,
    pub block: u64,
    pub orders: Vec<Order>,
    #[serde_as(as = "BTreeMap<_, HexOrDecimalU256>")]
    pub prices: BTreeMap<Address, U256>,
}

#[serde_as]
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Order {
    #[serde_as(as = "serde_ext::Hex")]
    pub uid: Vec<u8>,
    pub sell_token: Address,
    pub buy_token: Address,
    #[serde_as(as = "HexOrDecimalU256")]
    pub sell_amount: U256,
    #[serde_as(as = "HexOrDecimalU256")]
    pub buy_amount: U256,
    pub kind: OrderKind,
    pub class: OrderClass,
    #[serde(default)]
    pub partially_fillable: bool,
    pub valid_to: u32,
}

#[derive(Debug, Deserialize, Clone, Copy)]
#[serde(rename_all = "camelCase")]
pub enum OrderKind {
    Sell,
    Buy,
}

#[derive(Debug, Deserialize, Clone, Copy)]
#[serde(rename_all = "camelCase")]
pub enum OrderClass {
    Market,
    Limit,
    Liquidity,
}
