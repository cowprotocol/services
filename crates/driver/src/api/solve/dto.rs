use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct Auction {
    pub id: u64,
    pub block: u64,
    pub orders: u64,
    pub prices: u64,
}

#[derive(Debug, Deserialize)]
pub struct Order {}
