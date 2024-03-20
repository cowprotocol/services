//! A 0x-based trade finder.

use {
    super::{Interaction, Quote, Trade, TradeError, TradeFinding},
    crate::{
        price_estimation::{gas, Query},
        request_sharing::{BoxRequestSharing, BoxShared, RequestSharing},
        zeroex_api::{SwapQuery, ZeroExApi, ZeroExResponseError},
    },
    anyhow::Context,
    futures::FutureExt as _,
    model::order::OrderKind,
    primitive_types::H160,
    std::sync::Arc,
};

pub struct ZeroExTradeFinder {
    inner: Inner,
    sharing: BoxRequestSharing<Query, Result<Trade, TradeError>>,
}

#[derive(Clone)]
struct Inner {
    api: Arc<dyn ZeroExApi>,
    excluded_sources: Vec<String>,
    buy_only: bool,
    solver: H160,
}

impl ZeroExTradeFinder {
    pub fn new(
        api: Arc<dyn ZeroExApi>,
        excluded_sources: Vec<String>,
        buy_only: bool,
        solver: H160,
    ) -> Self {
        Self {
            inner: Inner {
                api,
                excluded_sources,
                buy_only,
                solver,
            },
            sharing: RequestSharing::labelled("zeroex".into()),
        }
    }

    fn shared_quote(&self, query: &Query) -> BoxShared<Result<Trade, TradeError>> {
        self.sharing.shared_or_else(query.clone(), |_| {
            let inner = self.inner.clone();
            let query = query.clone();
            async move { inner.quote(&query).await }.boxed()
        })
    }
}

impl Inner {
    async fn quote(&self, query: &Query) -> Result<Trade, TradeError> {
        if self.buy_only && query.kind == OrderKind::Sell {
            return Err(TradeError::UnsupportedOrderType("sell order".to_string()));
        }

        let (sell_amount, buy_amount) = match query.kind {
            OrderKind::Buy => (None, Some(query.in_amount)),
            OrderKind::Sell => (Some(query.in_amount), None),
        };

        let swap = self
            .api
            .get_swap(
                SwapQuery {
                    sell_token: query.sell_token,
                    buy_token: query.buy_token,
                    sell_amount: sell_amount.map(|amount| amount.get()),
                    buy_amount: buy_amount.map(|amount| amount.get()),
                    slippage_percentage: None,
                    taker_address: None,
                    excluded_sources: self.excluded_sources.clone(),
                    intent_on_filling: false,
                    enable_slippage_protection: false,
                },
                query.block_dependent,
            )
            .await?;

        Ok(Trade::swap(
            query.sell_token,
            match query.kind {
                OrderKind::Buy => swap.price.sell_amount,
                OrderKind::Sell => swap.price.buy_amount,
            },
            gas::SETTLEMENT_OVERHEAD + swap.price.estimated_gas,
            Some(swap.price.allowance_target),
            Interaction {
                target: swap.to,
                value: swap.value,
                data: swap.data,
            },
            self.solver,
        ))
    }
}

#[async_trait::async_trait]
impl TradeFinding for ZeroExTradeFinder {
    async fn get_quote(&self, query: &Query) -> Result<Quote, TradeError> {
        let trade = self.shared_quote(query).await?;
        let gas_estimate = trade
            .gas_estimate
            .context("no gas estimate")
            .map_err(TradeError::Other)?;
        Ok(Quote {
            out_amount: trade.out_amount,
            gas_estimate,
            solver: self.inner.solver,
        })
    }

    async fn get_trade(&self, query: &Query) -> Result<Trade, TradeError> {
        self.shared_quote(query).await
    }
}

impl From<ZeroExResponseError> for TradeError {
    fn from(err: ZeroExResponseError) -> Self {
        match err {
            ZeroExResponseError::InsufficientLiquidity => TradeError::NoLiquidity,
            ZeroExResponseError::RateLimited => TradeError::RateLimited,
            ZeroExResponseError::ServerError(_)
            | ZeroExResponseError::UnknownZeroExError(_)
            | ZeroExResponseError::DeserializeError(_, _)
            | ZeroExResponseError::TextFetch(_)
            | ZeroExResponseError::Send(_) => TradeError::Other(err.into()),
        }
    }
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        crate::zeroex_api::{DefaultZeroExApi, MockZeroExApi, PriceResponse, SwapResponse},
        hex_literal::hex,
        number::nonzero::U256 as NonZeroU256,
        std::time::Duration,
    };

    fn create_trader(api: Arc<dyn ZeroExApi>) -> ZeroExTradeFinder {
        ZeroExTradeFinder::new(api, Default::default(), false, H160([1; 20]))
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
                    to: addr!("def1c0ded9bec7f1a1670819833240f027b25eff"),
                    value: 42.into(),
                    data: vec![1, 2, 3, 4],
                })
            }
            .boxed()
        });

        let weth = testlib::tokens::WETH;
        let gno = testlib::tokens::GNO;

        let trader = create_trader(Arc::new(zeroex_api));

        let trade = trader
            .get_trade(&Query {
                verification: None,
                sell_token: weth,
                buy_token: gno,
                in_amount: NonZeroU256::try_from(100000000000000000u128).unwrap(),
                kind: OrderKind::Sell,
                block_dependent: false,
            })
            .await
            .unwrap();

        assert_eq!(trade.out_amount, 1110165823572443613u64.into());
        assert!(trade.gas_estimate.unwrap() > 111000);
        assert_eq!(
            trade.interactions,
            vec![
                Interaction {
                    target: weth,
                    value: 0.into(),
                    data: hex!(
                        "095ea7b3
                         000000000000000000000000def1c0ded9bec7f1a1670819833240f027b25eff
                         0000000000000000000000000000000000000000000000000000000000000000"
                    )
                    .to_vec(),
                },
                Interaction {
                    target: weth,
                    value: 0.into(),
                    data: hex!(
                        "095ea7b3
                         000000000000000000000000def1c0ded9bec7f1a1670819833240f027b25eff
                         ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff"
                    )
                    .to_vec(),
                },
                Interaction {
                    target: addr!("def1c0ded9bec7f1a1670819833240f027b25eff"),
                    value: 42.into(),
                    data: vec![1, 2, 3, 4],
                }
            ]
        );
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
                    data: vec![5, 6, 7, 8],
                    ..Default::default()
                })
            }
            .boxed()
        });

        let weth = testlib::tokens::WETH;
        let gno = testlib::tokens::GNO;

        let trader = create_trader(Arc::new(zeroex_api));

        let trade = trader
            .get_trade(&Query {
                verification: None,
                sell_token: weth,
                buy_token: gno,
                in_amount: NonZeroU256::try_from(100000000000000000u128).unwrap(),
                kind: OrderKind::Buy,
                block_dependent: false,
            })
            .await
            .unwrap();

        assert_eq!(trade.out_amount, 8986186353137488u64.into());
        assert!(trade.gas_estimate.unwrap() > 111000);
        assert_eq!(trade.interactions.len(), 3);
        assert_eq!(trade.interactions[2].data, [5, 6, 7, 8]);
    }

    #[tokio::test]
    async fn shares_quote_api_request() {
        let mut zeroex_api = MockZeroExApi::new();
        zeroex_api.expect_get_swap().return_once(|_, _| {
            async move {
                tokio::time::sleep(Duration::from_millis(1)).await;
                Ok(Default::default())
            }
            .boxed()
        });

        let trader = create_trader(Arc::new(zeroex_api));

        let query = Query::default();
        let result = futures::try_join!(trader.get_quote(&query), trader.get_trade(&query));

        assert!(result.is_ok());
    }

    #[tokio::test]
    #[ignore]
    async fn real_estimate() {
        let weth = testlib::tokens::WETH;
        let gno = testlib::tokens::GNO;

        let zeroex_api = DefaultZeroExApi::test();
        let trader = create_trader(Arc::new(zeroex_api));

        let trade = trader
            .get_trade(&Query {
                verification: None,
                sell_token: weth,
                buy_token: gno,
                in_amount: NonZeroU256::try_from(10u128).unwrap(),
                kind: OrderKind::Sell,
                block_dependent: false,
            })
            .await
            .unwrap();

        let gno = trade.out_amount.to_f64_lossy() / 1e18;
        println!("1.0 ETH buys {gno} GNO");
        println!("gas:  {:?}", trade.gas_estimate);
        for interaction in trade.interactions {
            println!("data: 0x{}", hex::encode(interaction.data));
        }
    }
}
