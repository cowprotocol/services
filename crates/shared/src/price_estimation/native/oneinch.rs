use {
    super::{NativePrice, NativePriceEstimateResult, NativePriceEstimating},
    crate::{price_estimation::PriceEstimationError, token_info::TokenInfoFetching},
    anyhow::{anyhow, Context, Result},
    ethrpc::block_stream::{into_stream, CurrentBlockWatcher},
    futures::{future::BoxFuture, FutureExt, StreamExt},
    num::ToPrimitive,
    number::{conversions::u256_to_big_rational, serialization::HexOrDecimalU256},
    primitive_types::{H160, U256},
    reqwest::{header::AUTHORIZATION, Client},
    serde::Deserialize,
    serde_with::serde_as,
    std::{
        collections::HashMap,
        sync::{Arc, Mutex},
    },
    url::Url,
};

#[serde_as]
#[derive(Debug, Deserialize)]
struct Response(#[serde_as(as = "HashMap<_, HexOrDecimalU256>")] HashMap<H160, U256>);

type Token = H160;

pub struct OneInch {
    prices: Arc<Mutex<HashMap<Token, NativePrice>>>,
}

impl OneInch {
    pub fn new(
        client: Client,
        base_url: Url,
        api_key: Option<String>,
        chain_id: u64,
        current_block: CurrentBlockWatcher,
        token_info: Arc<dyn TokenInfoFetching>,
    ) -> Self {
        let instance = Self {
            prices: Arc::new(Mutex::new(HashMap::new())),
        };
        instance.update_prices_in_background(
            client,
            base_url,
            api_key,
            chain_id,
            current_block,
            token_info,
        );
        instance
    }

    fn update_prices_in_background(
        &self,
        client: Client,
        base_url: Url,
        api_key: Option<String>,
        chain_id: u64,
        current_block: CurrentBlockWatcher,
        token_info: Arc<dyn TokenInfoFetching>,
    ) {
        let prices = self.prices.clone();
        tokio::task::spawn(async move {
            let mut block_stream = into_stream(current_block);
            loop {
                let current_prices = get_current_prices(
                    &client,
                    base_url.clone(),
                    api_key.clone(),
                    chain_id,
                    token_info.as_ref(),
                )
                .await;

                match current_prices {
                    Ok(current_prices) => {
                        tracing::debug!("OneInch spot prices updated");
                        *prices.lock().unwrap() = current_prices;
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
            prices
                .get(&token)
                .cloned()
                .ok_or_else(|| PriceEstimationError::NoLiquidity)
        }
        .boxed()
    }
}

async fn get_current_prices(
    client: &Client,
    base_url: Url,
    api_key: Option<String>,
    chain: u64,
    token_info: &dyn TokenInfoFetching,
) -> Result<HashMap<Token, f64>> {
    let url = crate::url::join(&base_url, &format!("/price/v1.1/{}", chain));
    let mut builder = client.get(url);
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
    let prices = serde_json::from_str::<Response>(&response)
        .with_context(|| format!("Failed to parse Native 1inch prices from {response:?}"))?
        .0;

    let token_infos = token_info
        .get_token_infos(&prices.keys().copied().collect::<Vec<_>>())
        .await;

    let normalized_prices = prices
        .into_iter()
        .filter_map(|(token, price)| {
            let Some(decimals) = token_infos.get(&token).and_then(|info| info.decimals) else {
                tracing::debug!(?token, "could not fetch decimals; discarding spot price");
                return None;
            };
            let unit =
                num::BigRational::from_integer(num::BigInt::from(10u64).pow(decimals.into()));
            let normalized_price = u256_to_big_rational(&price) / unit;
            Some((token, normalized_price.to_f64()?))
        })
        .collect();
    Ok(normalized_prices)
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        crate::token_info::{MockTokenInfoFetching, TokenInfo},
        std::{env, str::FromStr},
    };

    const BASE_URL: &str = "https://api.1inch.dev/";

    #[tokio::test]
    #[ignore]
    async fn works() {
        let auth_token = env::var("ONEINCH_AUTH_TOKEN").unwrap();

        let mut token_info = MockTokenInfoFetching::new();
        token_info.expect_get_token_infos().returning(|tokens| {
            tokens
                .iter()
                .map(|token| {
                    (
                        *token,
                        TokenInfo {
                            symbol: None,
                            // hard code 6 decimals because we are testing with USDC
                            decimals: Some(6),
                        },
                    )
                })
                .collect()
        });

        let prices = get_current_prices(
            &Client::default(),
            Url::parse(BASE_URL).unwrap(),
            Some(auth_token),
            1,
            &token_info,
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
