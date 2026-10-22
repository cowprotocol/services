//! Bitget swap-API adapter.
//!
//! One signed `/swap` call per sell order. Buys are unsupported: `/swap` only
//! quotes exact-in.

mod dto;

use {
    super::{Error, Order, Side, Swap},
    crate::config::BitgetConfig,
    base64::prelude::*,
    hmac::{Hmac, Mac},
    sha2::Sha256,
    solana_sdk::pubkey::Pubkey,
    std::collections::BTreeMap,
};

const SWAP_PATH: &str = "swap";
/// The path Bitget checks the signature against, whichever proxy `endpoint`
/// points at.
const SIGNED_SWAP_PATH: &str = "/bgw-pro/swapx/pro/swap";
const PARTNER_CODE: &str = "cowswap";

/// Adapter over Bitget's swap API.
pub struct Bitget {
    client: reqwest::Client,
    endpoint: reqwest::Url,
    api_key: String,
    api_secret: String,
    slippage_bps: u16,
    max_accounts: u16,
}

impl Bitget {
    pub fn new(config: &BitgetConfig) -> Result<Self, Error> {
        Ok(Self {
            client: reqwest::Client::builder().build()?,
            endpoint: config.endpoint.clone(),
            api_key: config.api_key.clone(),
            api_secret: config.api_secret.clone(),
            slippage_bps: config.slippage_bps,
            max_accounts: config.max_accounts,
        })
    }

    /// Quote `order` for the settlement signer `taker` and return the swap to
    /// run inside the settlement transaction.
    pub async fn swap(&self, order: &Order, taker: &Pubkey) -> Result<Swap, Error> {
        if order.side == Side::Buy {
            return Err(Error::OrderNotSupported);
        }
        let url = self
            .endpoint
            .join(SWAP_PATH)
            .map_err(|_| Error::RequestBuildFailed)?;
        let request = dto::SwapRequest::new(order, taker, self.slippage_bps, self.max_accounts);
        let body = serde_json::to_string(&request).map_err(|_| Error::RequestBuildFailed)?;
        let timestamp = chrono::Utc::now().timestamp_millis().to_string();
        let response = self
            .client
            .post(url)
            .header("content-type", "application/json")
            .header("Partner-Code", PARTNER_CODE)
            .header("x-api-key", &self.api_key)
            .header("x-api-timestamp", &timestamp)
            .header("x-api-signature", self.signature(&body, &timestamp))
            .body(body)
            .send()
            .await?;
        let status = response.status();
        let body = response.text().await?;
        if status == reqwest::StatusCode::TOO_MANY_REQUESTS {
            return Err(Error::RateLimited);
        }
        if !status.is_success() {
            return Err(Error::Api {
                status: status.as_u16(),
                body,
            });
        }
        serde_json::from_str::<dto::Envelope>(&body)
            .map_err(|err| Error::BadResponse(format!("response body: {err}")))?
            .into_data(status.as_u16())?
            .into_swap(order, taker)
    }

    /// Base64 HMAC-SHA256, keyed by the secret, over the signed fields as JSON
    /// with sorted keys.
    fn signature(&self, body: &str, timestamp: &str) -> String {
        let content = serde_json::to_string(&BTreeMap::from([
            ("apiPath", SIGNED_SWAP_PATH),
            ("body", body),
            ("x-api-key", self.api_key.as_str()),
            ("x-api-timestamp", timestamp),
        ]))
        .expect("a string map serializes");
        let mut mac = Hmac::<Sha256>::new_from_slice(self.api_secret.as_bytes())
            .expect("HMAC takes keys of any length");
        mac.update(content.as_bytes());
        BASE64_STANDARD.encode(mac.finalize().into_bytes())
    }
}

#[cfg(test)]
mod tests {
    use {super::*, std::str::FromStr};

    fn config(api_key: String, api_secret: String) -> BitgetConfig {
        BitgetConfig {
            endpoint: "https://bopenapi.bgwapi.io/bgw-pro/swapx/pro/"
                .parse()
                .unwrap(),
            api_key,
            api_secret,
            slippage_bps: 20,
            max_accounts: 48,
        }
    }

    /// Live Bitget API. Needs `BITGET_API_KEY` and `BITGET_API_SECRET`.
    #[tokio::test]
    #[ignore]
    async fn bitget_live_sell() {
        let bitget = Bitget::new(&config(
            std::env::var("BITGET_API_KEY").unwrap(),
            std::env::var("BITGET_API_SECRET").unwrap(),
        ))
        .unwrap();
        // 1 USDC for wSOL, paid into the settlement's wSOL buffer.
        let order = Order {
            sell_mint: Pubkey::from_str("EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v").unwrap(),
            buy_mint: Pubkey::from_str("So11111111111111111111111111111111111111112").unwrap(),
            buy_destination: Pubkey::from_str("4BAJ8YzUCw2H7CXgmCZdMG2jFLaFdGjREhxRqXma73Cv")
                .unwrap(),
            sell_amount: 1_000_000,
            buy_amount: 0,
            side: Side::Sell,
        };
        let taker = Pubkey::new_unique();
        let swap = bitget.swap(&order, &taker).await.unwrap();

        assert_eq!(swap.in_amount, 1_000_000);
        assert!(swap.out_amount > 0);
        let transfer = swap.instructions.last().unwrap();
        assert_eq!(transfer.accounts[1].pubkey, order.buy_destination);

        let solution = crate::dto::solution::Solution::new(
            0,
            crate::dto::order::OrderUid([1; 32]),
            &order,
            swap,
        )
        .unwrap();
        serde_json::to_string(&solution).unwrap();
    }
}
