//! Bitget swap-API adapter.
//!
//! One signed `/swap` call per sell order. Buys are unsupported: `/swap` only
//! quotes exact-in. Fee-exempt routes also need `/signBgwSolTx` at settle
//! time.

mod dto;

use {
    super::{Error, Order, Side, Swap},
    crate::config::{BitgetConfig, Fee},
    base64::prelude::*,
    hmac::{Hmac, Mac},
    serde::{Serialize, de::DeserializeOwned},
    sha2::Sha256,
    solana_sdk::{pubkey::Pubkey, transaction::VersionedTransaction},
    std::collections::BTreeMap,
};

/// The path prefix Bitget checks signatures against, whichever proxy
/// `endpoint` points at.
const SIGNED_PATH: &str = "/bgw-pro/swapx/pro/";
const SWAP_PATH: &str = "swap";
const COSIGN_PATH: &str = "signBgwSolTx";
const PARTNER_CODE: &str = "cowswap";

/// Adapter over Bitget's swap API.
pub struct Bitget {
    client: reqwest::Client,
    endpoint: reqwest::Url,
    api_key: String,
    api_secret: String,
    slippage_bps: u16,
    max_accounts: u16,
    fee: Fee,
}

impl Bitget {
    pub fn new(config: &BitgetConfig) -> Result<Self, Error> {
        let mut endpoint = config.endpoint.clone();
        // `join` would replace a last path segment that lacks the slash.
        if !endpoint.path().ends_with('/') {
            endpoint.set_path(&format!("{}/", endpoint.path()));
        }
        Ok(Self {
            client: reqwest::Client::builder().build()?,
            endpoint,
            api_key: config.api_key.clone(),
            api_secret: config.api_secret.clone(),
            slippage_bps: config.slippage_bps,
            max_accounts: config.max_accounts,
            fee: config.fee,
        })
    }

    /// Quote `order` for the settlement signer `taker` and return the swap to
    /// run inside the settlement transaction.
    pub async fn swap(&self, order: &Order, taker: &Pubkey) -> Result<Swap, Error> {
        if order.side == Side::Buy {
            return Err(Error::OrderNotSupported);
        }
        let request =
            dto::SwapRequest::new(order, taker, self.slippage_bps, self.max_accounts, self.fee);
        self.post::<dto::SwapResponse>(SWAP_PATH, &request)
            .await?
            .into_swap(order, taker, self.fee)
    }

    /// Add Bitget's signature to a transaction carrying a fee-exempt route.
    pub async fn cosign(
        &self,
        transaction: &VersionedTransaction,
    ) -> Result<VersionedTransaction, Error> {
        self.post::<dto::CosignResponse>(COSIGN_PATH, &dto::CosignRequest::new(transaction))
            .await?
            .into_transaction()
    }

    async fn post<T: DeserializeOwned>(
        &self,
        path: &str,
        request: &impl Serialize,
    ) -> Result<T, Error> {
        let url = self
            .endpoint
            .join(path)
            .map_err(|_| Error::RequestBuildFailed)?;
        let body = serde_json::to_string(request).map_err(|_| Error::RequestBuildFailed)?;
        let timestamp = chrono::Utc::now().timestamp_millis().to_string();
        let signature = self.signature(&format!("{SIGNED_PATH}{path}"), &body, &timestamp);
        let response = self
            .client
            .post(url)
            .header("content-type", "application/json")
            .header("Partner-Code", PARTNER_CODE)
            .header("x-api-key", &self.api_key)
            .header("x-api-timestamp", &timestamp)
            .header("x-api-signature", signature)
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
            .into_data(status.as_u16())
    }

    /// Base64 HMAC-SHA256, keyed by the secret, over the signed fields as JSON
    /// with sorted keys.
    fn signature(&self, api_path: &str, body: &str, timestamp: &str) -> String {
        let content = serde_json::to_string(&BTreeMap::from([
            ("apiPath", api_path),
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
    use {
        super::*,
        solana_sdk::{
            hash::Hash,
            message::{VersionedMessage, v0},
            signature::Signature,
        },
        std::str::FromStr,
    };

    fn live_config() -> BitgetConfig {
        BitgetConfig {
            endpoint: "https://bopenapi.bgwapi.io/bgw-pro/swapx/pro/"
                .parse()
                .unwrap(),
            api_key: std::env::var("BITGET_API_KEY").unwrap(),
            api_secret: std::env::var("BITGET_API_SECRET").unwrap(),
            slippage_bps: 20,
            max_accounts: 48,
            fee: Fee::Standard,
        }
    }

    /// 1 USDC for wSOL, paid into the settlement's wSOL buffer.
    fn usdc_for_wsol() -> Order {
        Order {
            sell_mint: Pubkey::from_str("EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v").unwrap(),
            buy_mint: Pubkey::from_str("So11111111111111111111111111111111111111112").unwrap(),
            buy_destination: Pubkey::from_str("4BAJ8YzUCw2H7CXgmCZdMG2jFLaFdGjREhxRqXma73Cv")
                .unwrap(),
            sell_amount: 1_000_000,
            buy_amount: 0,
            side: Side::Sell,
        }
    }

    /// Live Bitget API. Needs `BITGET_API_KEY` and `BITGET_API_SECRET`.
    #[tokio::test]
    #[ignore]
    async fn bitget_live_cosign() {
        let bitget = Bitget::new(&BitgetConfig {
            fee: Fee::Exempt,
            ..live_config()
        })
        .unwrap();
        let taker = Pubkey::new_unique();
        let swap = bitget.swap(&usdc_for_wsol(), &taker).await.unwrap();
        let message = VersionedMessage::V0(
            v0::Message::try_compile(&taker, &swap.instructions, &[], Hash::default()).unwrap(),
        );
        let transaction = VersionedTransaction {
            signatures: vec![Signature::default(); 2],
            message,
        };

        let cosigned = bitget.cosign(&transaction).await.unwrap();

        assert_eq!(cosigned.message, transaction.message);
        // The taker's slot stays empty, Bitget's is signed.
        assert_eq!(cosigned.verify_with_results(), [false, true]);
    }

    /// Live Bitget API. Needs `BITGET_API_KEY` and `BITGET_API_SECRET`.
    #[tokio::test]
    #[ignore]
    async fn bitget_live_sell() {
        let bitget = Bitget::new(&live_config()).unwrap();
        let order = usdc_for_wsol();
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
