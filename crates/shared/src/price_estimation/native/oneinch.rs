use {
    super::{NativePriceEstimateResult, NativePriceEstimating},
    crate::price_estimation::PriceEstimationError,
    anyhow::{anyhow, Result},
    ethrpc::current_block::{into_stream, CurrentBlockStream},
    futures::{future::BoxFuture, FutureExt, StreamExt},
    primitive_types::{H160, U256},
    reqwest::Client,
    std::{
        collections::HashMap,
        sync::{Arc, Mutex},
    },
};

const BASE_URL: &str = "https://api.1inch.dev/";

struct OneInch {
    // Denominated in wei
    prices: Arc<Mutex<HashMap<H160, U256>>>,
}

impl OneInch {
    #[allow(dead_code)]
    pub fn new(client: Client, chain_id: u64, current_block: CurrentBlockStream) -> Self {
        let instance = Self {
            prices: Arc::new(Mutex::new(HashMap::new())),
        };
        instance.update_prices_in_background(client, chain_id, current_block);
        instance
    }

    fn update_prices_in_background(
        &self,
        client: Client,
        chain_id: u64,
        current_block: CurrentBlockStream,
    ) {
        let prices = self.prices.clone();
        tokio::task::spawn(async move {
            let mut block_stream = into_stream(current_block);
            loop {
                match update_prices(&client, chain_id).await {
                    Ok(new_prices) => {
                        tracing::debug!("OneInch spot prices updated");
                        *prices.lock().unwrap() = new_prices;
                    }
                    Err(err) => {
                        tracing::warn!(?err, "OneInch spot price update failed");
                    }
                }
                block_stream.next().await;
            }
        });
    }
}

impl NativePriceEstimating for OneInch {
    fn estimate_native_price(&self, token: H160) -> BoxFuture<'_, NativePriceEstimateResult> {
        async move {
            let prices = self.prices.lock().unwrap();
            let price = prices
                .get(&token)
                .ok_or_else(|| PriceEstimationError::NoLiquidity)?;
            Ok(price.to_f64_lossy() / 1e18)
        }
        .boxed()
    }
}

async fn update_prices(client: &Client, chain: u64) -> Result<HashMap<H160, U256>> {
    let result = client
        .get(format!("{}/price/v1.1/{}", BASE_URL, chain))
        .send()
        .await
        .map_err(|err| anyhow!("Failed to fetch Native 1inch prices: {}", err))?
        .json::<HashMap<H160, String>>()
        .await
        .map_err(|err| anyhow!("Failed to parse Native 1inch prices: {}", err))?
        .into_iter()
        .filter_map(|(key, value)| match U256::from_dec_str(&value) {
            Ok(value) => Some((key, value)),
            Err(err) => {
                tracing::error!(%err, %value, "Failed to parse Native 1inch price");
                None
            }
        })
        .collect();
    Ok(result)
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        reqwest::header,
        std::{env, str::FromStr},
    };

    #[tokio::test]
    #[ignore]
    async fn works() {
        let mut headers = header::HeaderMap::new();
        let auth_token = env::var("ONEINCH_AUTH_TOKEN").unwrap();
        let mut auth_value = header::HeaderValue::from_str(&auth_token).unwrap();
        auth_value.set_sensitive(true);
        headers.insert(header::AUTHORIZATION, auth_value);

        let client = reqwest::Client::builder()
            .default_headers(headers)
            .build()
            .unwrap();

        let prices = update_prices(&client, 1).await.unwrap();
        assert!(prices.len() > 0);

        let native_token = H160::from_str("0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2").unwrap();
        let instance = OneInch {
            prices: Arc::new(Mutex::new(prices)),
        };
        assert_eq!(
            instance.estimate_native_price(native_token).await.unwrap(),
            1.
        );

        // Inverse price of USDC is >100 (this will fail if the price ETH goes below
        // $100)
        assert!(
            1. / instance
                .estimate_native_price(
                    H160::from_str("0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48").unwrap()
                )
                .await
                .unwrap()
                > 100.
        );
    }
}
