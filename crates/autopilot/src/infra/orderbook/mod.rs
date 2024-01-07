//! A client to the CoW Protocol public API.

use reqwest::Url;

pub mod auction;

pub struct Orderbook {
    client: reqwest::Client,
    url: Url,
}

impl Orderbook {
    /// Creates a new CoW Protocol client.
    pub fn new(client: reqwest::Client, url: Url) -> Self {
        Self { client, url }
    }
}
