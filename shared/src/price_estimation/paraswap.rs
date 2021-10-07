use super::{ensure_token_supported, Estimate, PriceEstimating, PriceEstimationError, Query};
use crate::{
    bad_token::BadTokenDetecting,
    paraswap_api::{ParaswapApi, ParaswapResponseError, PriceQuery, Side},
    token_info::{TokenInfo, TokenInfoFetching},
};
use anyhow::{anyhow, Context, Result};
use model::order::OrderKind;
use primitive_types::H160;
use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
};

pub struct ParaswapPriceEstimator {
    pub paraswap: Arc<dyn ParaswapApi>,
    pub token_info: Arc<dyn TokenInfoFetching>,
    pub bad_token_detector: Arc<dyn BadTokenDetecting>,
    pub disabled_paraswap_dexs: Vec<String>,
}

impl ParaswapPriceEstimator {
    async fn estimate_(
        &self,
        query: &Query,
        token_infos: &HashMap<H160, TokenInfo>,
    ) -> Result<Estimate, PriceEstimationError> {
        if query.buy_token == query.sell_token {
            return Ok(Estimate {
                out_amount: query.in_amount,
                gas: 0.into(),
            });
        }

        ensure_token_supported(query.buy_token, self.bad_token_detector.as_ref()).await?;
        ensure_token_supported(query.sell_token, self.bad_token_detector.as_ref()).await?;

        let price_query = PriceQuery {
            src_token: query.sell_token,
            dest_token: query.buy_token,
            src_decimals: decimals(&query.sell_token, token_infos)? as usize,
            dest_decimals: decimals(&query.buy_token, token_infos)? as usize,
            amount: query.in_amount,
            side: match query.kind {
                OrderKind::Buy => Side::Buy,
                OrderKind::Sell => Side::Sell,
            },
            exclude_dexs: Some(self.disabled_paraswap_dexs.clone()),
        };

        let response = self
            .paraswap
            .price(price_query)
            .await
            .map_err(|err| match err {
                ParaswapResponseError::InsufficientLiquidity => PriceEstimationError::NoLiquidity,
                _ => PriceEstimationError::Other(err.into()),
            })
            .context("paraswap")?;
        Ok(Estimate {
            out_amount: match query.kind {
                OrderKind::Buy => response.src_amount,
                OrderKind::Sell => response.dest_amount,
            },
            gas: response.gas_cost,
        })
    }
}

fn decimals(
    token: &H160,
    token_infos: &HashMap<H160, TokenInfo>,
) -> Result<u8, PriceEstimationError> {
    token_infos
        .get(token)
        .and_then(|info| info.decimals)
        .ok_or_else(|| PriceEstimationError::Other(anyhow!("failed to get decimals")))
}

#[async_trait::async_trait]
impl PriceEstimating for ParaswapPriceEstimator {
    async fn estimates(&self, queries: &[Query]) -> Vec<Result<Estimate, PriceEstimationError>> {
        let tokens = queries
            .iter()
            .flat_map(|query| [query.sell_token, query.buy_token])
            .collect::<HashSet<_>>()
            .into_iter()
            .collect::<Vec<_>>();
        let token_infos = self.token_info.get_token_infos(&tokens).await;
        // TODO: concurrency?
        let mut results = Vec::new();
        for query in queries {
            results.push(self.estimate_(query, &token_infos).await);
        }
        results
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        bad_token::list_based::ListBasedDetector, paraswap_api::DefaultParaswapApi,
        token_info::MockTokenInfoFetching,
    };
    use reqwest::Client;

    #[tokio::test]
    #[ignore]
    async fn real_estimate() {
        let mut token_info = MockTokenInfoFetching::new();
        token_info.expect_get_token_infos().returning(|tokens| {
            tokens
                .iter()
                .map(|token| {
                    (
                        *token,
                        TokenInfo {
                            decimals: Some(18),
                            symbol: Some("SYM".to_string()),
                        },
                    )
                })
                .collect()
        });
        let paraswap = DefaultParaswapApi {
            client: Client::new(),
            partner: "".to_string(),
        };
        let detector = ListBasedDetector::deny_list(Vec::new());
        let estimator = ParaswapPriceEstimator {
            paraswap: Arc::new(paraswap),
            token_info: Arc::new(token_info),
            bad_token_detector: Arc::new(detector),
            disabled_paraswap_dexs: Vec::new(),
        };

        let weth = addr!("c02aaa39b223fe8d0a0e5c4f27ead9083c756cc2");
        let gno = addr!("6810e776880c02933d47db1b9fc05908e5386b96");
        let query = Query {
            sell_token: weth,
            buy_token: gno,
            in_amount: 10u128.pow(18).into(),
            kind: OrderKind::Sell,
        };

        let result = estimator.estimate(&query).await;
        dbg!(&result);
        let estimate = result.unwrap();
        println!(
            "1 eth buys {} gno",
            estimate.out_amount.to_f64_lossy() / 1e18
        );
        // You can compare this to
        // <api url>/api/v1/markets/c02aaa39b223fe8d0a0e5c4f27ead9083c756cc2-6810e776880c02933d47db1b9fc05908e5386b96/sell/1000000000000000000
    }
}
