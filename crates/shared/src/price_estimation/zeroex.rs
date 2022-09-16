use super::{trade_finder::TradeEstimator, PriceEstimateResult, PriceEstimating, Query};
use crate::{
    rate_limiter::RateLimiter, trade_finding::zeroex::ZeroExTradeFinder, zeroex_api::ZeroExApi,
};
use std::sync::Arc;

pub struct ZeroExPriceEstimator(TradeEstimator);

impl ZeroExPriceEstimator {
    pub fn new(
        api: Arc<dyn ZeroExApi>,
        excluded_sources: Vec<String>,
        rate_limiter: Arc<RateLimiter>,
    ) -> Self {
        Self(TradeEstimator::new(
            Arc::new(ZeroExTradeFinder::new(api, excluded_sources)),
            rate_limiter,
        ))
    }
}

impl PriceEstimating for ZeroExPriceEstimator {
    fn estimates<'a>(
        &'a self,
        queries: &'a [Query],
    ) -> futures::stream::BoxStream<'_, (usize, PriceEstimateResult)> {
        self.0.estimates(queries)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        price_estimation::single_estimate,
        zeroex_api::{DefaultZeroExApi, MockZeroExApi, PriceResponse, SwapResponse},
    };
    use model::order::OrderKind;
    use reqwest::Client;

    fn create_estimator(api: Arc<dyn ZeroExApi>) -> ZeroExPriceEstimator {
        ZeroExPriceEstimator::new(
            api,
            Default::default(),
            Arc::new(RateLimiter::from_strategy(
                Default::default(),
                "test".into(),
            )),
        )
    }

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

        let estimator = create_estimator(Arc::new(zeroex_api));

        let est = single_estimate(
            &estimator,
            &Query {
                sell_token: weth,
                buy_token: gno,
                in_amount: 100000000000000000u64.into(),
                kind: OrderKind::Sell,
            },
        )
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

        let estimator = create_estimator(Arc::new(zeroex_api));

        let est = single_estimate(
            &estimator,
            &Query {
                sell_token: weth,
                buy_token: gno,
                in_amount: 100000000000000000u64.into(),
                kind: OrderKind::Buy,
            },
        )
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

        let zeroex_api = DefaultZeroExApi::with_default_url(Client::new());
        let estimator = create_estimator(Arc::new(zeroex_api));

        let result = single_estimate(
            &estimator,
            &Query {
                sell_token: weth,
                buy_token: gno,
                in_amount: 10u128.pow(18).into(),
                kind: OrderKind::Sell,
            },
        )
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
