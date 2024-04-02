use {
    super::{
        trade_finder::TradeEstimator,
        trade_verifier::TradeVerifying,
        PriceEstimateResult,
        PriceEstimating,
        Query,
    },
    crate::{trade_finding::zeroex::ZeroExTradeFinder, zeroex_api::ZeroExApi},
    primitive_types::H160,
    rate_limit::RateLimiter,
    std::sync::Arc,
};

pub struct ZeroExPriceEstimator(TradeEstimator);

impl ZeroExPriceEstimator {
    pub fn new(
        api: Arc<dyn ZeroExApi>,
        excluded_sources: Vec<String>,
        rate_limiter: Arc<RateLimiter>,
        buy_only: bool,
        solver: H160,
    ) -> Self {
        Self(TradeEstimator::new(
            Arc::new(ZeroExTradeFinder::new(
                api,
                excluded_sources,
                buy_only,
                solver,
            )),
            rate_limiter,
            "zeroex".into(),
        ))
    }

    pub fn verified(&self, verifier: Arc<dyn TradeVerifying>) -> Self {
        Self(self.0.clone().with_verifier(verifier))
    }
}

impl PriceEstimating for ZeroExPriceEstimator {
    fn estimate(&self, query: Arc<Query>) -> futures::future::BoxFuture<'_, PriceEstimateResult> {
        tracing::info!("newlog 0xPriceEstimator query={:?}", query);
        self.0.estimate(query)
    }
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        crate::{
            price_estimation::PriceEstimationError,
            zeroex_api::{DefaultZeroExApi, MockZeroExApi, PriceResponse, SwapResponse},
        },
        ethcontract::futures::FutureExt as _,
        model::order::OrderKind,
        number::nonzero::U256 as NonZeroU256,
    };

    fn create_estimator(api: Arc<dyn ZeroExApi>, buy_only: bool) -> ZeroExPriceEstimator {
        ZeroExPriceEstimator::new(
            api,
            Default::default(),
            Arc::new(RateLimiter::from_strategy(
                Default::default(),
                "test".into(),
            )),
            buy_only,
            H160([1; 20]),
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
        zeroex_api.expect_get_swap().return_once(|_, _| {
            async move {
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
            }
            .boxed()
        });

        let weth = testlib::tokens::WETH;
        let gno = testlib::tokens::GNO;

        let estimator = create_estimator(Arc::new(zeroex_api), false);

        let est = estimator
            .estimate(Arc::new(Query {
                verification: None,
                sell_token: weth,
                buy_token: gno,
                in_amount: NonZeroU256::try_from(100000000000000000u128).unwrap(),
                kind: OrderKind::Sell,
                block_dependent: false,
            }))
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
        zeroex_api.expect_get_swap().return_once(|_, _| {
            async move {
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
            }
            .boxed()
        });

        let weth = testlib::tokens::WETH;
        let gno = testlib::tokens::GNO;

        let estimator = create_estimator(Arc::new(zeroex_api), false);

        let est = estimator
            .estimate(Arc::new(Query {
                verification: None,
                sell_token: weth,
                buy_token: gno,
                in_amount: NonZeroU256::try_from(100000000000000000u128).unwrap(),
                kind: OrderKind::Buy,
                block_dependent: false,
            }))
            .await
            .unwrap();

        assert_eq!(est.out_amount, 8986186353137488u64.into());
        assert!(est.gas > 111000);
    }

    #[tokio::test]
    async fn filter_out_sell_estimates() {
        let mut zeroex_api = MockZeroExApi::new();

        zeroex_api.expect_get_swap().return_once(|_, _| {
            async move {
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
            }
            .boxed()
        });

        let weth = testlib::tokens::WETH;
        let gno = testlib::tokens::GNO;

        let estimator = create_estimator(Arc::new(zeroex_api), true);

        let result = estimator
            .estimate(Arc::new(Query {
                verification: None,
                sell_token: weth,
                buy_token: gno,
                in_amount: NonZeroU256::try_from(100000000000000000u128).unwrap(),
                kind: OrderKind::Sell,
                block_dependent: false,
            }))
            .await;

        assert!(matches!(
            &result,
            Err(PriceEstimationError::UnsupportedOrderType(_))
        ));

        let result = estimator
            .estimate(Arc::new(Query {
                verification: None,
                sell_token: weth,
                buy_token: gno,
                in_amount: NonZeroU256::try_from(100000000000000000u128).unwrap(),
                kind: OrderKind::Buy,
                block_dependent: false,
            }))
            .await;

        assert!(matches!(
            &result,
            Ok(est) if est.out_amount.as_u64() == 8986186353137488u64
        ));
    }

    #[tokio::test]
    #[ignore]
    async fn real_estimate() {
        let weth = testlib::tokens::WETH;
        let gno = testlib::tokens::GNO;

        let zeroex_api = DefaultZeroExApi::test();
        let estimator = create_estimator(Arc::new(zeroex_api), false);

        let result = estimator
            .estimate(Arc::new(Query {
                verification: None,
                sell_token: weth,
                buy_token: gno,
                in_amount: NonZeroU256::try_from(10u128.pow(18)).unwrap(),
                kind: OrderKind::Sell,
                block_dependent: false,
            }))
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
