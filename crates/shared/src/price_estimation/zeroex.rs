use crate::{
    price_estimation::{
        gas, Estimate, PriceEstimateResult, PriceEstimating, PriceEstimationError, Query,
    },
    request_sharing::RequestSharing,
    zeroex_api::{SwapQuery, SwapResponse, ZeroExApi},
};
use futures::{future::BoxFuture, FutureExt, StreamExt};
use model::order::OrderKind;
use std::sync::Arc;

pub struct ZeroExPriceEstimator {
    api: Arc<dyn ZeroExApi>,
    sharing: RequestSharing<Query, BoxFuture<'static, Result<SwapResponse, PriceEstimationError>>>,
    excluded_sources: Vec<String>,
}

impl ZeroExPriceEstimator {
    pub fn new(api: Arc<dyn ZeroExApi>, excluded_sources: Vec<String>) -> Self {
        Self {
            api,
            sharing: Default::default(),
            excluded_sources,
        }
    }

    async fn estimate(&self, query: &Query) -> Result<Estimate, PriceEstimationError> {
        let (sell_amount, buy_amount) = match query.kind {
            OrderKind::Buy => (None, Some(query.in_amount)),
            OrderKind::Sell => (Some(query.in_amount), None),
        };

        let swap_query = SwapQuery {
            sell_token: query.sell_token,
            buy_token: query.buy_token,
            sell_amount,
            buy_amount,
            slippage_percentage: Default::default(),
            excluded_sources: self.excluded_sources.clone(),
        };
        let api = self.api.clone();
        let swap_future = async move {
            api.get_swap(swap_query)
                .await
                .map_err(|err| PriceEstimationError::Other(err.into()))
        };
        let swap = self.sharing.shared(*query, swap_future.boxed()).await?;

        Ok(Estimate {
            out_amount: match query.kind {
                OrderKind::Buy => swap.price.sell_amount,
                OrderKind::Sell => swap.price.buy_amount,
            },
            gas: gas::SETTLEMENT_OVERHEAD + swap.price.estimated_gas,
        })
    }
}

impl PriceEstimating for ZeroExPriceEstimator {
    fn estimates<'a>(
        &'a self,
        queries: &'a [Query],
    ) -> futures::stream::BoxStream<'_, (usize, PriceEstimateResult)> {
        debug_assert!(queries.iter().all(|query| {
            query.buy_token != model::order::BUY_ETH_ADDRESS
                && query.sell_token != model::order::BUY_ETH_ADDRESS
                && query.sell_token != query.buy_token
        }));

        futures::stream::iter(queries)
            .then(|query| self.estimate(query))
            .enumerate()
            .boxed()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::zeroex_api::{DefaultZeroExApi, PriceResponse};
    use crate::zeroex_api::{MockZeroExApi, SwapResponse};
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
        zeroex_api.expect_get_swap().return_once(|_| {
            Ok(SwapResponse {
                price: PriceResponse {
                    sell_amount: 100000000000000000u64.into(),
                    buy_amount: 1110165823572443613u64.into(),
                    allowance_target: addr!("def1c0ded9bec7f1a1670819833240f027b25eff"),
                    price: 11.101_658_235_724_436,
                    estimated_gas: 111000,
                },
                ..Default::default()
            })
        });

        let weth = testlib::tokens::WETH;
        let gno = testlib::tokens::GNO;

        let estimator = ZeroExPriceEstimator {
            api: Arc::new(zeroex_api),
            sharing: Default::default(),
            excluded_sources: Default::default(),
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
        assert!(est.gas > 111000);
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
        zeroex_api.expect_get_swap().return_once(|_| {
            Ok(SwapResponse {
                price: PriceResponse {
                    sell_amount: 8986186353137488u64.into(),
                    buy_amount: 100000000000000000u64.into(),
                    allowance_target: addr!("def1c0ded9bec7f1a1670819833240f027b25eff"),
                    price: 0.089_861_863_531_374_87,
                    estimated_gas: 111000,
                },
                ..Default::default()
            })
        });

        let weth = testlib::tokens::WETH;
        let gno = testlib::tokens::GNO;

        let estimator = ZeroExPriceEstimator {
            api: Arc::new(zeroex_api),
            sharing: Default::default(),
            excluded_sources: Default::default(),
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
        assert!(est.gas > 111000);
    }

    #[tokio::test]
    #[ignore]
    async fn real_estimate() {
        let weth = testlib::tokens::WETH;
        let gno = testlib::tokens::GNO;

        let estimator = ZeroExPriceEstimator {
            api: Arc::new(DefaultZeroExApi::with_default_url(Client::new())),
            sharing: Default::default(),
            excluded_sources: Default::default(),
        };

        let result = estimator
            .estimate(&Query {
                sell_token: weth,
                buy_token: gno,
                in_amount: 10u128.pow(18).into(),
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
