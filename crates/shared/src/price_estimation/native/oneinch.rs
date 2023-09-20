use {
    super::{NativePriceEstimateResult, NativePriceEstimating},
    crate::price_estimation::PriceEstimationError,
    anyhow::{anyhow, Context, Result},
    ethrpc::current_block::{into_stream, CurrentBlockStream},
    futures::{future::BoxFuture, FutureExt, StreamExt},
    number::u256_decimal::DecimalU256,
    primitive_types::{H160, U256},
    reqwest::{header::AUTHORIZATION, Client},
    serde::{Deserialize, Serialize},
    serde_with::serde_as,
    std::{
        collections::HashMap,
        sync::{Arc, Mutex},
    },
    url::Url,
};

const BASE_URL: &str = "https://api.1inch.dev/";

#[serde_as]
#[derive(Debug, Deserialize, Serialize)]
struct Response(#[serde_as(as = "HashMap<_, DecimalU256>")] HashMap<Token, PriceInWei>);

type Token = H160;
type PriceInWei = U256;
pub struct OneInch {
    prices: Arc<Mutex<HashMap<Token, PriceInWei>>>,
}

impl OneInch {
    #[allow(dead_code)]
    pub fn new(
        client: Client,
        base_url: Option<Url>,
        api_key: Option<String>,
        chain_id: u64,
        current_block: CurrentBlockStream,
    ) -> Self {
        let instance = Self {
            prices: Arc::new(Mutex::new(HashMap::new())),
        };
        instance.update_prices_in_background(
            client,
            base_url.unwrap_or(Url::parse(BASE_URL).unwrap()),
            api_key,
            chain_id,
            current_block,
        );
        instance
    }

    fn update_prices_in_background(
        &self,
        client: Client,
        base_url: Url,
        api_key: Option<String>,
        chain_id: u64,
        current_block: CurrentBlockStream,
    ) {
        let prices = self.prices.clone();
        tokio::task::spawn(async move {
            let mut block_stream = into_stream(current_block);
            loop {
                match update_prices(&client, base_url.clone(), api_key.clone(), chain_id).await {
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
    fn estimate_native_price(&self, token: Token) -> BoxFuture<'_, NativePriceEstimateResult> {
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

async fn update_prices(
    client: &Client,
    base_url: Url,
    api_key: Option<String>,
    chain: u64,
) -> Result<HashMap<Token, PriceInWei>> {
    let mut builder = client.get(format!("{}/price/v1.1/{}", base_url, chain));
    if let Some(api_key) = api_key {
        builder = builder.header(AUTHORIZATION, api_key)
    }
    let response = builder
        .send()
        .await
        .context("Failed to send Native 1inch price request")?;
    if !response.status().is_success() {
        return Err(anyhow!(
            "Native 1inch price request failed with status {}",
            response.status()
        ));
    }
    let response = response
        .text()
        .await
        .context("Failed to fetch Native 1Inch prices")?;
    let result = serde_json::from_str::<Response>(&response)
        .with_context(|| format!("Failed to parse Native 1inch prices from {response:?}"))?;
    Ok(result.0)
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        std::{env, str::FromStr},
    };

    #[tokio::test]
    #[ignore]
    async fn works() {
        let auth_token = env::var("ONEINCH_AUTH_TOKEN").unwrap();

        let prices = update_prices(
            &Client::default(),
            Url::parse(BASE_URL).unwrap(),
            Some(auth_token),
            1,
        )
        .await
        .unwrap();
        assert!(!prices.is_empty());

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
