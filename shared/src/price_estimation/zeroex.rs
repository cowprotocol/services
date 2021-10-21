use crate::bad_token::BadTokenDetecting;
use crate::price_estimation::{
    ensure_token_supported, Estimate, PriceEstimating, PriceEstimationError, Query,
};
use crate::zeroex_api::{SwapQuery, ZeroExApi};
use model::order::OrderKind;
use std::sync::Arc;

pub struct ZeroExPriceEstimator {
    pub client: Arc<dyn ZeroExApi + Send + Sync>,
    pub bad_token_detector: Arc<dyn BadTokenDetecting>,
}

impl ZeroExPriceEstimator {
    async fn estimate(&self, query: &Query) -> Result<Estimate, PriceEstimationError> {
        if query.buy_token == query.sell_token {
            return Ok(Estimate {
                out_amount: query.in_amount,
                gas: 0.into(),
            });
        }

        ensure_token_supported(query.buy_token, self.bad_token_detector.as_ref()).await?;
        ensure_token_supported(query.sell_token, self.bad_token_detector.as_ref()).await?;

        let (sell_amount, buy_amount) = match query.kind {
            OrderKind::Buy => (None, Some(query.in_amount)),
            OrderKind::Sell => (Some(query.in_amount), None),
        };

        let swap = self
            .client
            .get_price(SwapQuery {
                sell_token: query.sell_token,
                buy_token: query.buy_token,
                sell_amount,
                buy_amount,
                slippage_percentage: Default::default(),
                skip_validation: Some(true),
            })
            .await
            .map_err(|err| PriceEstimationError::Other(err.into()))?;

        Ok(Estimate {
            out_amount: match query.kind {
                OrderKind::Buy => swap.sell_amount,
                OrderKind::Sell => swap.buy_amount,
            },
            gas: swap.estimated_gas,
        })
    }
}

#[async_trait::async_trait]
impl PriceEstimating for ZeroExPriceEstimator {
    async fn estimates(
        &self,
        queries: &[Query],
    ) -> Vec<anyhow::Result<Estimate, PriceEstimationError>> {
        let mut results = Vec::with_capacity(queries.len());

        for query in queries {
            results.push(self.estimate(query).await);
        }

        results
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bad_token::list_based::ListBasedDetector;
    use crate::zeroex_api::MockZeroExApi;
    use crate::zeroex_api::{DefaultZeroExApi, PriceResponse};
    use reqwest::Client;

    #[tokio::test]
    async fn estimate_sell() {
        let mut zeroex_api = MockZeroExApi::new();

        // Response was generated with:
        //
        // curl "https://api.0x.org/swap/v1/price?\
        //     sellToken=0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2&\
        //     buyToken=0x6810e776880c02933d47db1b9fc05908e5386b96&\
        //     slippagePercentage=0&\
        //     sellAmount=100000000000000000"
        zeroex_api.expect_get_price().return_once(|_| {
            Ok(PriceResponse {
                sell_amount: 100000000000000000u64.into(),
                buy_amount: 1110165823572443613u64.into(),
                allowance_target: addr!("def1c0ded9bec7f1a1670819833240f027b25eff"),
                price: 11.101_658_235_724_436,
                estimated_gas: 111000.into(),
            })
        });

        let weth = addr!("c02aaa39b223fe8d0a0e5c4f27ead9083c756cc2");
        let gno = addr!("6810e776880c02933d47db1b9fc05908e5386b96");

        let estimator = ZeroExPriceEstimator {
            client: Arc::new(zeroex_api),
            bad_token_detector: Arc::new(ListBasedDetector::deny_list(Vec::new())),
        };

        let est = estimator
            .estimate(&Query {
                sell_token: weth,
                buy_token: gno,
                in_amount: 100000000000000000u64.into(),
                kind: OrderKind::Sell,
            })
            .await
            .unwrap();

        assert_eq!(est.out_amount, 1110165823572443613u64.into());
        assert_eq!(est.gas, 111000.into());
    }

    #[tokio::test]
    async fn estimate_buy() {
        let mut zeroex_api = MockZeroExApi::new();

        // Response was generated with:
        //
        // curl "https://api.0x.org/swap/v1/price?\
        //     sellToken=0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2&\
        //     buyToken=0x6810e776880c02933d47db1b9fc05908e5386b96&\
        //     slippagePercentage=0&\
        //     buyAmount=100000000000000000"
        zeroex_api.expect_get_price().return_once(|_| {
            Ok(PriceResponse {
                sell_amount: 8986186353137488u64.into(),
                buy_amount: 100000000000000000u64.into(),
                allowance_target: addr!("def1c0ded9bec7f1a1670819833240f027b25eff"),
                price: 0.089_861_863_531_374_87,
                estimated_gas: 111000.into(),
            })
        });

        let weth = addr!("c02aaa39b223fe8d0a0e5c4f27ead9083c756cc2");
        let gno = addr!("6810e776880c02933d47db1b9fc05908e5386b96");

        let estimator = ZeroExPriceEstimator {
            client: Arc::new(zeroex_api),
            bad_token_detector: Arc::new(ListBasedDetector::deny_list(Vec::new())),
        };

        let est = estimator
            .estimate(&Query {
                sell_token: weth,
                buy_token: gno,
                in_amount: 100000000000000000u64.into(),
                kind: OrderKind::Buy,
            })
            .await
            .unwrap();

        assert_eq!(est.out_amount, 8986186353137488u64.into());
        assert_eq!(est.gas, 111000.into());
    }

    #[tokio::test]
    async fn same_token() {
        let estimator = ZeroExPriceEstimator {
            client: Arc::new(MockZeroExApi::new()),
            bad_token_detector: Arc::new(ListBasedDetector::deny_list(Vec::new())),
        };

        let weth = addr!("c02aaa39b223fe8d0a0e5c4f27ead9083c756cc2");

        let est = estimator
            .estimate(&Query {
                sell_token: weth,
                buy_token: weth,
                in_amount: 100000000000000000u64.into(),
                kind: OrderKind::Buy,
            })
            .await
            .unwrap();

        assert_eq!(est.out_amount, 100000000000000000u64.into());
        assert_eq!(est.gas, 0.into());
    }

    #[tokio::test]
    async fn unsupported_token() {
        let weth = addr!("c02aaa39b223fe8d0a0e5c4f27ead9083c756cc2");
        let gno = addr!("6810e776880c02933d47db1b9fc05908e5386b96");

        let estimator = ZeroExPriceEstimator {
            client: Arc::new(MockZeroExApi::new()),
            // we don't support this shady token -_-
            bad_token_detector: Arc::new(ListBasedDetector::deny_list(vec![gno])),
        };

        let err = estimator
            .estimate(&Query {
                sell_token: weth,
                buy_token: gno,
                in_amount: 100000000000000000u64.into(),
                kind: OrderKind::Buy,
            })
            .await
            .unwrap_err();

        if let PriceEstimationError::UnsupportedToken(token) = err {
            assert_eq!(token, gno);
        } else {
            panic!("unexpected error: {:?}", err);
        }
    }

    #[tokio::test]
    #[ignore]
    async fn real_estimate() {
        let weth = addr!("c02aaa39b223fe8d0a0e5c4f27ead9083c756cc2");
        let gno = addr!("6810e776880c02933d47db1b9fc05908e5386b96");

        let estimator = ZeroExPriceEstimator {
            client: Arc::new(DefaultZeroExApi::with_default_url(Client::new())),
            bad_token_detector: Arc::new(ListBasedDetector::deny_list(Vec::new())),
        };

        let result = estimator
            .estimate(&Query {
                sell_token: weth,
                buy_token: gno,
                in_amount: 100000000000000000u64.into(),
                kind: OrderKind::Sell,
            })
            .await;

        dbg!(&result);
        let estimate = result.unwrap();
        println!(
            "1 eth buys {} gno, costing {} gas",
            estimate.out_amount.to_f64_lossy() / 1e18,
            estimate.gas,
        );
    }
}
