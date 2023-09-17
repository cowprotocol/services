use {
    super::{NativePriceEstimateResult, NativePriceEstimating},
    crate::price_estimation::{
        Estimate,
        PriceEstimateResult,
        PriceEstimating,
        PriceEstimationError,
        Query,
    },
    anyhow::{anyhow, Result},
    futures::{future::BoxFuture, FutureExt},
    model::order::OrderKind,
    number::nonzero::U256 as NonZeroU256,
    primitive_types::{H160, U256},
    reqwest::Client,
    std::{
        collections::HashMap,
        sync::{Arc, Mutex},
        time::Duration,
    },
};

const BASE_URL: &str = "https://api.1inch.dev/";
const REFRESH_INTERVAL: Duration = Duration::from_secs(12);

struct OneInch {
    client: Client,
    chain_id: u64,
    // Denominated in wei
    prices: Arc<Mutex<HashMap<H160, U256>>>,
    native_price_estimation_amount: NonZeroU256,
    native_token: H160,
}

impl OneInch {
    #[allow(dead_code)]
    pub fn new(
        client: Client,
        chain_id: u64,
        native_price_estimation_amount: NonZeroU256,
        native_token: H160,
    ) -> Self {
        let instance = Self {
            client,
            chain_id,
            prices: Arc::new(Mutex::new(HashMap::new())),
            native_price_estimation_amount,
            native_token,
        };
        instance.update_prices_in_background();
        instance
    }

    fn update_prices_in_background(&self) {
        let client = self.client.clone();
        let chain = self.chain_id;
        let prices = self.prices.clone();
        tokio::task::spawn(async move {
            loop {
                match update_prices(&client, chain).await {
                    Ok(new_prices) => {
                        *prices.lock().unwrap() = new_prices;
                    }
                    Err(err) => {
                        tracing::warn!(?err);
                    }
                }
                tokio::time::sleep(REFRESH_INTERVAL).await;
            }
        });
    }
}

impl NativePriceEstimating for OneInch {
    fn estimate_native_price<'a>(
        &'a self,
        token: &'a H160,
    ) -> BoxFuture<'_, NativePriceEstimateResult> {
        async move {
            let prices = self.prices.lock().unwrap();
            let price = prices
                .get(token)
                .ok_or_else(|| PriceEstimationError::NoLiquidity)?;
            Ok(price.to_f64_lossy() / 1e18)
        }
        .boxed()
    }
}

impl PriceEstimating for OneInch {
    fn estimate(&self, query: Arc<Query>) -> BoxFuture<'_, PriceEstimateResult> {
        async move {
            if query.kind != OrderKind::Buy
                || query.in_amount != self.native_price_estimation_amount
                || query.buy_token != self.native_token
            {
                return Err(PriceEstimationError::UnsupportedOrderType(
                    "Non native price quote".to_string(),
                ));
            }

            let prices = self.prices.lock().unwrap();
            let price = prices
                .get(&query.sell_token)
                .ok_or_else(|| PriceEstimationError::NoLiquidity)?;
            let reverse_price = 1. / (price.to_f64_lossy() / 1e18);
            let in_amount: U256 = query.in_amount.into();
            Ok(Estimate {
                out_amount: U256::from_f64_lossy(in_amount.to_f64_lossy() * reverse_price),
                gas: 0,
                solver: H160([0; 20]),
            })
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
            chain_id: 1,
            client,
            prices: Arc::new(Mutex::new(prices)),
            native_price_estimation_amount: U256::exp10(18).try_into().unwrap(),
            native_token,
        };
        assert_eq!(
            instance.estimate_native_price(&native_token).await.unwrap(),
            1.
        );

        // Inverse price of USDC is >100 (this will fail if the price ETH goes below
        // $100)
        assert!(
            1. / instance
                .estimate_native_price(
                    &H160::from_str("0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48").unwrap()
                )
                .await
                .unwrap()
                > 100.
        );
    }

    #[tokio::test]
    async fn rejects_unsupported_quote_requests() {
        let native_token = H160::from_str("0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2").unwrap();
        let native_price_estimation_amount = U256::exp10(18).try_into().unwrap();
        let instance = OneInch {
            native_price_estimation_amount,
            native_token,
            chain_id: 1,
            client: Default::default(),
            prices: Default::default(),
        };

        // This query is good (but since we have no prices it yields NoLiquidity)
        let query = Query {
            sell_token: H160::from_low_u64_be(1),
            buy_token: native_token,
            in_amount: native_price_estimation_amount,
            kind: OrderKind::Buy,
            verification: None,
        };
        assert!(matches!(
            instance.estimate(Arc::new(query.clone())).await,
            Err(PriceEstimationError::NoLiquidity)
        ));

        // These queries are no allowed
        let mut sell_query = query.clone();
        sell_query.kind = OrderKind::Sell;
        assert!(matches!(
            instance.estimate(Arc::new(sell_query)).await,
            Err(PriceEstimationError::UnsupportedOrderType(_))
        ));

        let mut bad_amount_query = query.clone();
        bad_amount_query.in_amount = NonZeroU256::default();
        assert!(matches!(
            instance.estimate(Arc::new(bad_amount_query)).await,
            Err(PriceEstimationError::UnsupportedOrderType(_))
        ));

        let mut bad_buy_token_query = query.clone();
        bad_buy_token_query.buy_token = H160::from_low_u64_be(2);
        assert!(matches!(
            instance.estimate(Arc::new(bad_buy_token_query)).await,
            Err(PriceEstimationError::UnsupportedOrderType(_))
        ));
    }
}
