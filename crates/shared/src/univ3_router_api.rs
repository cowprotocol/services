//! Bindings for an instance of https://github.com/cowprotocol/univ3-api .

use anyhow::{Context, Result};
use model::u256_decimal;
use primitive_types::{H160, U256};
use reqwest::{Client, Url};
use serde::{Deserialize, Serialize};
use serde_with::{serde_as, DisplayFromStr};

#[derive(Debug, Copy, Clone, Serialize)]
pub enum Type {
    #[serde(rename = "buy")]
    Buy,
    #[serde(rename = "sell")]
    Sell,
}

#[derive(Debug, Copy, Clone, Serialize)]
pub struct Request {
    #[serde(rename = "type")]
    pub type_: Type,
    pub token_in: H160,
    pub token_out: H160,
    #[serde(with = "u256_decimal")]
    pub amount: U256,
    pub recipient: H160,
}

#[serde_as]
#[derive(Debug, Clone, Deserialize)]
pub struct Response {
    #[serde(with = "u256_decimal")]
    pub quote: U256,
    #[serde_as(as = "DisplayFromStr")]
    pub gas: u64,
    #[serde(with = "model::bytes_hex")]
    pub call_data: Vec<u8>,
}

pub struct Api {
    client: Client,
    estimate: Url,
}

impl Api {
    pub fn new(client: Client, base: Url) -> Self {
        Self {
            client,
            estimate: base.join("estimate").unwrap(),
        }
    }

    pub async fn request(&self, request: &Request) -> Result<Response> {
        self.client
            .post(self.estimate.clone())
            .json(request)
            .send()
            .await
            .context("send")?
            .json()
            .await
            .context("json")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serialize_request() {
        let request = Request {
            type_: Type::Sell,
            token_in: addr!("c02aaa39b223fe8d0a0e5c4f27ead9083c756cc2"),
            token_out: addr!("dac17f958d2ee523a2206206994597c13d831ec7"),
            amount: 1000000000000000000u64.into(),
            recipient: addr!("0000000000000000000000000000000000000000"),
        };
        let serialized = serde_json::to_value(request).unwrap();
        let expected = serde_json::json!({
            "type": "sell",
            "token_in": "0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2",
            "token_out": "0xdac17f958d2ee523a2206206994597c13d831ec7",
            "amount": "1000000000000000000",
            "recipient": "0x0000000000000000000000000000000000000000",
        });
        assert_eq!(serialized, expected);
    }

    #[test]
    fn deserialize_response() {
        let json = r#"
        {
            "quote": "284671676368727715",
            "gas": "113000",
            "call_data": "0x0102"
        }"#;
        let response: Response = serde_json::from_str(json).unwrap();
        assert_eq!(response.quote, 284671676368727715u64.into());
        assert_eq!(response.gas, 113000);
        assert_eq!(response.call_data, &[0x01, 0x02]);
    }

    #[tokio::test]
    #[ignore]
    async fn real() {
        let api = Api::new(Default::default(), "http://localhost:8080".parse().unwrap());
        let request = Request {
            type_: Type::Sell,
            token_in: addr!("c02aaa39b223fe8d0a0e5c4f27ead9083c756cc2"),
            token_out: addr!("dac17f958d2ee523a2206206994597c13d831ec7"),
            amount: 1000000000000000000u64.into(),
            recipient: addr!("0000000000000000000000000000000000000000"),
        };
        let response = api.request(&request).await.unwrap();
        println!("{response:?}");
    }
}
