use {
    super::{Query, Quote, TradeError, TradeFinding},
    crate::{
        paraswap_api::{
            ParaswapApi,
            ParaswapResponseError,
            PriceQuery,
            PriceResponse,
            Side,
            TradeAmount,
            TransactionBuilderQuery,
        },
        price_estimation::gas,
        request_sharing::{BoxRequestSharing, BoxShared, RequestSharing},
        token_info::{TokenInfo, TokenInfoFetching},
        trade_finding::{Interaction, Trade},
    },
    anyhow::{Context, Result},
    futures::FutureExt as _,
    model::order::OrderKind,
    primitive_types::H160,
    std::{collections::HashMap, sync::Arc},
};

pub struct ParaswapTradeFinder {
    inner: Inner,
    sharing: BoxRequestSharing<Query, Result<InternalQuote, TradeError>>,
}

#[derive(Clone)]
struct Inner {
    paraswap: Arc<dyn ParaswapApi>,
    tokens: Arc<dyn TokenInfoFetching>,
    disabled_paraswap_dexs: Vec<String>,
    solver: H160,
}

#[derive(Clone)]
struct InternalQuote {
    data: Quote,
    tokens: HashMap<H160, TokenInfo>,
    price: PriceResponse,
}

impl ParaswapTradeFinder {
    pub fn new(
        api: Arc<dyn ParaswapApi>,
        tokens: Arc<dyn TokenInfoFetching>,
        disabled_paraswap_dexs: Vec<String>,
        solver: H160,
    ) -> Self {
        Self {
            inner: Inner {
                paraswap: api,
                tokens,
                disabled_paraswap_dexs,
                solver,
            },
            sharing: RequestSharing::labelled("paraswap".into()),
        }
    }

    fn shared_quote(&self, query: &Query) -> BoxShared<Result<InternalQuote, TradeError>> {
        self.sharing.shared_or_else(query.clone(), |_| {
            let inner = self.inner.clone();
            let query = query.clone();
            async move { inner.quote(&query).await }.boxed()
        })
    }

    async fn trade(&self, query: &Query) -> Result<Trade, TradeError> {
        let quote = self.shared_quote(query).await?;
        self.inner.trade(query, quote).await
    }
}

impl Inner {
    // Default to 1% slippage - same as the ParaSwap UI.
    const DEFAULT_SLIPPAGE: u32 = 100;
    // Use a default non-zero user address, otherwise the API will return an
    // error.
    const DEFAULT_USER: H160 = addr!("BEeFbeefbEefbeEFbeEfbEEfBEeFbeEfBeEfBeef");

    async fn quote(&self, query: &Query) -> Result<InternalQuote, TradeError> {
        let tokens = self
            .tokens
            .get_token_infos(&[query.sell_token, query.buy_token])
            .await;

        let price_query = PriceQuery {
            src_token: query.sell_token,
            dest_token: query.buy_token,
            src_decimals: decimals(&tokens, &query.sell_token)?,
            dest_decimals: decimals(&tokens, &query.buy_token)?,
            amount: query.in_amount.get(),
            side: match query.kind {
                OrderKind::Buy => Side::Buy,
                OrderKind::Sell => Side::Sell,
            },
            exclude_dexs: Some(self.disabled_paraswap_dexs.clone()),
        };

        let price = self
            .paraswap
            .price(price_query, query.block_dependent)
            .await?;
        let quote = Quote {
            out_amount: match query.kind {
                OrderKind::Buy => price.src_amount,
                OrderKind::Sell => price.dest_amount,
            },
            gas_estimate: gas::SETTLEMENT_OVERHEAD + price.gas_cost,
            solver: self.solver,
        };

        Ok(InternalQuote {
            data: quote,
            tokens,
            price,
        })
    }

    async fn trade(&self, query: &Query, mut quote: InternalQuote) -> Result<Trade, TradeError> {
        let tx_query = TransactionBuilderQuery {
            src_token: query.sell_token,
            dest_token: query.buy_token,
            trade_amount: match query.kind {
                OrderKind::Buy => TradeAmount::Buy {
                    dest_amount: query.in_amount.get().into(),
                    slippage: Self::DEFAULT_SLIPPAGE,
                },
                OrderKind::Sell => TradeAmount::Sell {
                    src_amount: query.in_amount.get().into(),
                    slippage: Self::DEFAULT_SLIPPAGE,
                },
            },
            src_decimals: decimals(&quote.tokens, &query.sell_token)?,
            dest_decimals: decimals(&quote.tokens, &query.buy_token)?,
            price_route: quote.price.price_route_raw.take(),
            user_address: query
                .verification
                .as_ref()
                .map(|v| v.from)
                .unwrap_or(Self::DEFAULT_USER),
        };

        let tx = self
            .paraswap
            .transaction(tx_query, query.block_dependent)
            .await?;

        Ok(Trade::swap(
            query.sell_token,
            quote.data.out_amount,
            quote.data.gas_estimate,
            Some(quote.price.token_transfer_proxy),
            Interaction {
                target: tx.to,
                value: tx.value.into(),
                data: tx.data,
            },
            self.solver,
        ))
    }
}

#[async_trait::async_trait]
impl TradeFinding for ParaswapTradeFinder {
    async fn get_quote(&self, query: &Query) -> Result<Quote, TradeError> {
        let quote = self.shared_quote(query).await?;
        Ok(quote.data)
    }

    async fn get_trade(&self, query: &Query) -> Result<Trade, TradeError> {
        self.trade(query).await
    }
}

impl From<ParaswapResponseError> for TradeError {
    fn from(err: ParaswapResponseError) -> Self {
        match err {
            ParaswapResponseError::InsufficientLiquidity(_) => Self::NoLiquidity,
            ParaswapResponseError::RateLimited => Self::RateLimited,
            ParaswapResponseError::Request(_)
            | ParaswapResponseError::Json(_)
            | ParaswapResponseError::Retryable(_)
            | ParaswapResponseError::Other(_) => Self::Other(err.into()),
        }
    }
}

fn decimals(tokens: &HashMap<H160, TokenInfo>, token: &H160) -> Result<u8, TradeError> {
    Ok(tokens
        .get(token)
        .and_then(|info| info.decimals)
        .context("failed to get decimals")?)
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        crate::{
            ethrpc::{create_env_test_transport, Web3},
            paraswap_api::{DefaultParaswapApi, MockParaswapApi},
            token_info::{MockTokenInfoFetching, TokenInfoFetcher},
        },
        ethrpc::current_block::BlockInfo,
        maplit::hashmap,
        number::nonzero::U256 as NonZeroU256,
        reqwest::Client,
        std::time::Duration,
        tokio::sync::watch,
    };

    #[tokio::test]
    async fn shares_prices_api_request() {
        let mut paraswap = MockParaswapApi::new();
        paraswap.expect_price().return_once(|_, _| {
            async move {
                tokio::time::sleep(Duration::from_millis(1)).await;
                Ok(Default::default())
            }
            .boxed()
        });
        paraswap
            .expect_transaction()
            .return_once(|_, _| async { Ok(Default::default()) }.boxed());

        let mut tokens = MockTokenInfoFetching::new();
        tokens.expect_get_token_infos().returning(|_| {
            hashmap! {
                H160::default() => TokenInfo {
                    decimals: Some(18),
                    ..Default::default()
                },
            }
        });

        let trader = ParaswapTradeFinder::new(
            Arc::new(paraswap),
            Arc::new(tokens),
            Vec::new(),
            H160([1; 20]),
        );

        let query = Query::default();
        let result = futures::try_join!(trader.get_quote(&query), trader.get_trade(&query));

        assert!(result.is_ok());
    }

    #[tokio::test]
    #[ignore]
    async fn real_trade() {
        let web3 = Web3::new(create_env_test_transport());
        let tokens = TokenInfoFetcher { web3: web3.clone() };
        let (_, block_stream) = watch::channel(BlockInfo::default());
        let paraswap = DefaultParaswapApi {
            client: Client::new(),
            base_url: "https://apiv5.paraswap.io".to_string(),
            partner: "Test".to_string(),
            block_stream,
        };
        let finder = ParaswapTradeFinder::new(
            Arc::new(paraswap),
            Arc::new(tokens),
            Vec::new(),
            H160([1; 20]),
        );

        let trade = finder
            .get_trade(&Query {
                verification: None,
                sell_token: testlib::tokens::WETH,
                buy_token: testlib::tokens::COW,
                in_amount: NonZeroU256::try_from(10u128.pow(18)).unwrap(),
                kind: OrderKind::Sell,
                block_dependent: false,
            })
            .await
            .unwrap();

        println!("1 ETH buys {} COW", trade.out_amount.to_f64_lossy() / 1e18);
    }
}
