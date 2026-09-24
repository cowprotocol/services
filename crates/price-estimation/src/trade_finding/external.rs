//! A trade finder that uses an external driver.

use {
    crate::{
        PriceEstimationError,
        Query,
        quote_id::QuoteIdAllocator,
        trade_finding::{
            Interaction,
            LegacyTrade,
            Quote,
            QuoteExecution,
            Trade,
            TradeError,
            TradeFinding,
            TradeKind,
            map_interactions_data,
        },
        trade_verifier::PriceQuery,
    },
    anyhow::{Context, anyhow},
    ethrpc::block_stream::CurrentBlockWatcher,
    futures::FutureExt,
    model::quote::QuoteId,
    observe::tracing::distributed::headers::tracing_headers,
    request_sharing::{BoxRequestSharing, RequestSharing},
    reqwest::{Client, StatusCode, header},
    std::sync::Arc,
    tracing::instrument,
    url::Url,
};

/// Wraps a trade result with the request ID of the HTTP request that produced
/// it, so that consumers reusing a shared in-flight request can identify the
/// original request.
#[derive(Clone)]
struct SharedTradeResponse {
    result: Result<TradeKind, PriceEstimationError>,
    request_id: Option<String>,
}

pub struct ExternalTradeFinder {
    /// URL to call to in the driver to get a quote with call data for a trade.
    quote_endpoint: Url,

    /// URL to call for fast-path quotes, so the solution is cached by the
    /// solver config that later settles it. Falls back to the `quote_endpoint`
    /// driver when a solver has no dedicated solve endpoint.
    fast_path_quote_endpoint: Url,

    /// Utility to make sure no 2 identical requests are in-flight at the same
    /// time. Instead of issuing a duplicated request this awaits the
    /// response of the in-flight request. See [`Self::shared_query`] for how
    /// quote ids are handed out in that case.
    sharing: BoxRequestSharing<Query, SharedTradeResponse>,

    /// Client to issue http requests with.
    client: Client,

    /// Stream to retrieve latest block information for block-dependent queries.
    block_stream: CurrentBlockWatcher,

    /// Where the ids quote requests are sent with come from.
    quote_ids: Arc<QuoteIdAllocator>,
}

impl ExternalTradeFinder {
    pub fn new(
        driver: Url,
        fast_path_driver: Option<Url>,
        client: Client,
        block_stream: CurrentBlockWatcher,
        quote_ids: Arc<QuoteIdAllocator>,
    ) -> Self {
        let fast_path_driver = fast_path_driver.unwrap_or_else(|| driver.clone());
        Self {
            quote_endpoint: crate::utils::join_url(&driver, "quote"),
            fast_path_quote_endpoint: crate::utils::join_url(&fast_path_driver, "quote"),
            sharing: RequestSharing::labelled(format!("tradefinder_{driver}")),
            client,
            block_stream,
            quote_ids,
        }
    }

    /// Queries the `/quote` endpoint of the configured driver and deserializes
    /// the result into a Quote or Trade.
    ///
    /// Identical in-flight queries share one driver request. The driver only
    /// learns the quote id of the caller that sent it; a caller joining that
    /// request gets the same answer but a fresh id of its own once the response
    /// is in, because its quote is stored as a separate row with its own gas
    /// price, native prices and verification result. Fast-path queries are
    /// never shared: the driver caches one settleable solution per quote id
    /// and every order needs its own.
    async fn shared_query(&self, query: &Query) -> Result<TradeKind, TradeError> {
        let fut = move |query: &Query| {
            let query = query.clone();
            let id = observe::tracing::distributed::request_id::from_current_span();
            let client = self.client.clone();
            let quote_endpoint = if query.fast_path {
                self.fast_path_quote_endpoint.clone()
            } else {
                self.quote_endpoint.clone()
            };
            let quote_ids = self.quote_ids.clone();
            let block_hash = self.block_stream.borrow().hash;

            async move {
                let result = async {
                    // Allocated inside the shared future so that only the
                    // request which actually goes out consumes an id.
                    let quote_id = quote_ids.next().await.map_err(|err| {
                        PriceEstimationError::ProtocolInternal(
                            err.context("failed to allocate quote id"),
                        )
                    })?;
                    let order = dto::Order {
                        sell_token: query.sell_token,
                        buy_token: query.buy_token,
                        amount: query.in_amount.get(),
                        kind: query.kind,
                        deadline: chrono::Utc::now() + query.timeout,
                        enable_fast_path: query.fast_path,
                        quote_id,
                    };

                    let mut request = client
                        .get(quote_endpoint)
                        .timeout(query.timeout)
                        .query(&order)
                        .headers(tracing_headers())
                        .header(header::CONTENT_TYPE, "application/json")
                        .header(header::ACCEPT, "application/json");
                    if query.block_dependent {
                        request = request.header("X-Current-Block-Hash", block_hash.to_string())
                    }
                    if let Some(ref id) = id {
                        request = request.header("X-REQUEST-ID", id.clone());
                    }

                    let response = request
                        .send()
                        .await
                        .map_err(|err| PriceEstimationError::EstimatorInternal(anyhow!(err)))?;
                    if response.status() == StatusCode::TOO_MANY_REQUESTS {
                        return Err(PriceEstimationError::RateLimited);
                    }
                    let text = response
                        .text()
                        .await
                        .map_err(|err| PriceEstimationError::EstimatorInternal(anyhow!(err)))?;
                    let result = serde_json::from_str::<dto::QuoteKind>(&text)
                        .map(|quote| TradeKind::from_dto(quote, quote_id))
                        .map_err(|err| {
                            serde_json::from_str::<dto::Error>(&text)
                                .map(PriceEstimationError::from)
                                .unwrap_or_else(|_| {
                                    PriceEstimationError::EstimatorInternal(anyhow!(err))
                                })
                        });

                    if order.enable_fast_path
                        && result
                            .as_ref()
                            .is_ok_and(|quote| !quote.supports_fast_path())
                    {
                        Err(PriceEstimationError::UnsupportedOrderType(
                            "solver does not support fast path".to_string(),
                        ))
                    } else {
                        result
                    }
                }
                .await;

                SharedTradeResponse {
                    result,
                    request_id: id,
                }
            }
            .boxed()
        };

        let response = if query.fast_path {
            fut(query).await
        } else {
            let shared = self.sharing.shared_or_else(query.clone(), fut);
            let is_shared = shared.is_shared;
            let mut response = shared.await;
            if is_shared {
                tracing::debug!(
                    original_request_id = ?response.request_id,
                    "reusing in-flight quote request"
                );
                if let Ok(trade) = &mut response.result {
                    let quote_id = self.quote_ids.next().await.map_err(|err| {
                        TradeError::Other(err.context("failed to allocate quote id"))
                    })?;
                    trade.set_quote_id(quote_id);
                }
            }
            response
        };

        response.result.map_err(TradeError::from)
    }
}

impl TradeKind {
    /// The driver's response as a trade, carrying the quote id the request
    /// was sent with.
    fn from_dto(quote: dto::QuoteKind, quote_id: QuoteId) -> Self {
        match quote {
            dto::QuoteKind::Legacy(quote) => Self::Legacy(LegacyTrade::from_dto(quote, quote_id)),
            dto::QuoteKind::Regular(quote) => Self::Regular(Trade::from_dto(quote, quote_id)),
        }
    }
}

impl LegacyTrade {
    fn from_dto(quote: dto::LegacyQuote, quote_id: QuoteId) -> Self {
        Self {
            out_amount: quote.amount,
            gas_estimate: quote.gas,
            interactions: quote.interactions.into_iter().map(Into::into).collect(),
            solver: quote.solver,
            tx_origin: quote.tx_origin,
            supports_fast_path: quote.supports_fast_path,
            quote_id,
        }
    }
}

impl Trade {
    fn from_dto(quote: dto::Quote, quote_id: QuoteId) -> Self {
        Self {
            clearing_prices: quote.clearing_prices,
            gas_estimate: quote.gas,
            pre_interactions: quote.pre_interactions.into_iter().map(Into::into).collect(),
            interactions: quote.interactions.into_iter().map(Into::into).collect(),
            solver: quote.solver,
            tx_origin: quote.tx_origin,
            jit_orders: quote.jit_orders,
            supports_fast_path: quote.supports_fast_path,
            quote_id,
        }
    }
}

impl From<dto::Error> for PriceEstimationError {
    fn from(value: dto::Error) -> Self {
        match value.kind.as_str() {
            "QuotingFailed" => Self::NoLiquidity,
            "TradingOutsideAllowedWindow" => Self::TradingOutsideAllowedWindow {
                message: value.description,
            },
            "TokenTemporarilySuspended" => Self::TokenTemporarilySuspended {
                message: value.description,
            },
            "InsufficientLiquidity" => Self::InsufficientLiquidity {
                message: value.description,
            },
            "CustomSolverError" => Self::CustomSolverError {
                message: value.description,
            },
            _ => Self::EstimatorInternal(anyhow!("{}", value.description)),
        }
    }
}

impl From<dto::Interaction> for Interaction {
    fn from(interaction: dto::Interaction) -> Self {
        Self {
            target: interaction.target,
            value: interaction.value,
            data: interaction.call_data,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_known_custom_error_kinds() {
        let cases = [
            (
                "TradingOutsideAllowedWindow",
                "window closed",
                "TradingOutsideAllowedWindow",
            ),
            (
                "TokenTemporarilySuspended",
                "token suspended",
                "TokenTemporarilySuspended",
            ),
            (
                "InsufficientLiquidity",
                "not enough liquidity",
                "InsufficientLiquidity",
            ),
            (
                "CustomSolverError",
                "custom solver reason",
                "CustomSolverError",
            ),
            ("QuotingFailed", "ignored", "QuotingFailed"),
        ];

        for (kind, description, expected) in cases {
            let error = dto::Error {
                kind: kind.to_string(),
                description: description.to_string(),
            };

            let mapped = PriceEstimationError::from(error);
            match expected {
                "TradingOutsideAllowedWindow" => {
                    assert!(matches!(
                        mapped,
                        PriceEstimationError::TradingOutsideAllowedWindow { message }
                        if message == description
                    ));
                }
                "TokenTemporarilySuspended" => {
                    assert!(matches!(
                        mapped,
                        PriceEstimationError::TokenTemporarilySuspended { message }
                        if message == description
                    ));
                }
                "InsufficientLiquidity" => {
                    assert!(matches!(
                        mapped,
                        PriceEstimationError::InsufficientLiquidity { message }
                        if message == description
                    ));
                }
                "CustomSolverError" => {
                    assert!(matches!(
                        mapped,
                        PriceEstimationError::CustomSolverError { message }
                        if message == description
                    ));
                }
                "QuotingFailed" => {
                    assert!(matches!(mapped, PriceEstimationError::NoLiquidity));
                }
                _ => unreachable!(),
            }
        }
    }

    /// Spawns a driver stand-in that records the `/quote` query strings it
    /// receives, answers every request with the same legacy quote after a short
    /// delay (so that concurrent callers overlap), and returns its URL.
    async fn spawn_mock_driver(hits: Arc<std::sync::Mutex<Vec<String>>>) -> Url {
        let app = axum::Router::new().route(
            "/quote",
            axum::routing::get(
                move |axum::extract::RawQuery(query): axum::extract::RawQuery| {
                    let hits = hits.clone();
                    async move {
                        hits.lock().unwrap().push(query.unwrap_or_default());
                        tokio::time::sleep(std::time::Duration::from_millis(300)).await;
                        axum::Json(serde_json::json!({
                            "amount": "2000",
                            "interactions": [],
                            "solver": "0x0000000000000000000000000000000000000001",
                            "gas": 1000,
                            "supportsFastPath": true,
                        }))
                    }
                },
            ),
        );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        format!("http://{addr}").parse().unwrap()
    }

    /// A finder whose ids are 1, 2, 3, ... in allocation order.
    fn finder(driver: Url, fast_path_driver: Option<Url>) -> ExternalTradeFinder {
        let mut generator = crate::MockQuoteIdGenerating::new();
        generator
            .expect_generate()
            .returning(|n| async move { Ok((1..=i64::try_from(n).unwrap()).collect()) }.boxed());
        ExternalTradeFinder::new(
            driver,
            fast_path_driver,
            Client::new(),
            ethrpc::block_stream::mock_single_block(Default::default()),
            Arc::new(QuoteIdAllocator::new(Arc::new(generator))),
        )
    }

    fn query(fast_path: bool) -> Query {
        Query {
            sell_token: alloy::primitives::Address::repeat_byte(1),
            buy_token: alloy::primitives::Address::repeat_byte(2),
            in_amount: number::nonzero::NonZeroU256::try_from(1000).unwrap(),
            kind: model::order::OrderKind::Sell,
            verification: Default::default(),
            block_dependent: false,
            fast_path,
            timeout: std::time::Duration::from_secs(5),
        }
    }

    fn quote_ids_sent(hits: &std::sync::Mutex<Vec<String>>) -> Vec<String> {
        hits.lock()
            .unwrap()
            .iter()
            .map(|query| {
                query
                    .split('&')
                    .find_map(|pair| pair.strip_prefix("quoteId="))
                    .expect("every request carries a quote id")
                    .to_owned()
            })
            .collect()
    }

    /// Two identical concurrent queries share one driver request. The driver
    /// only sees the first caller's id; the joining caller gets the shared
    /// answer under a fresh id of its own.
    #[tokio::test]
    async fn shared_request_hands_the_joining_caller_its_own_quote_id() {
        let hits = Arc::new(std::sync::Mutex::new(Vec::new()));
        let finder = finder(spawn_mock_driver(hits.clone()).await, None);
        let query = query(false);

        let (first, second) = tokio::join!(finder.get_trade(&query), finder.get_trade(&query));
        let (first, second) = (first.unwrap(), second.unwrap());

        assert_eq!(quote_ids_sent(&hits), vec!["1"]);
        let mut ids = [first.quote_id(), second.quote_id()];
        ids.sort();
        assert_eq!(ids, [1, 2]);
    }

    /// Fast-path queries are never shared: the driver caches one settleable
    /// solution per quote id, so every caller sends its own request.
    #[tokio::test]
    async fn fast_path_requests_are_not_shared() {
        let hits = Arc::new(std::sync::Mutex::new(Vec::new()));
        let finder = finder(spawn_mock_driver(hits.clone()).await, None);
        let query = query(true);

        let (first, second) = tokio::join!(finder.get_trade(&query), finder.get_trade(&query));
        let (first, second) = (first.unwrap(), second.unwrap());

        let mut sent = quote_ids_sent(&hits);
        sent.sort();
        assert_eq!(sent, vec!["1", "2"]);
        let mut ids = [first.quote_id(), second.quote_id()];
        ids.sort();
        assert_eq!(ids, [1, 2]);
    }

    /// A fast-path query is dispatched to the fast-path (solve) driver; a
    /// normal query goes to the default quote driver.
    #[tokio::test]
    async fn fast_path_query_uses_the_fast_path_endpoint() {
        let quote_hits = Arc::new(std::sync::Mutex::new(Vec::new()));
        let solve_hits = Arc::new(std::sync::Mutex::new(Vec::new()));
        let quote_driver = spawn_mock_driver(quote_hits.clone()).await;
        let solve_driver = spawn_mock_driver(solve_hits.clone()).await;
        let finder = finder(quote_driver, Some(solve_driver));

        finder.get_trade(&query(true)).await.unwrap();
        assert_eq!(solve_hits.lock().unwrap().len(), 1);
        assert!(quote_hits.lock().unwrap().is_empty());

        finder.get_trade(&query(false)).await.unwrap();
        assert_eq!(quote_hits.lock().unwrap().len(), 1);
        assert_eq!(solve_hits.lock().unwrap().len(), 1);
    }

    #[test]
    fn maps_unknown_error_kind_to_estimator_internal() {
        let error = dto::Error {
            kind: "SomeFutureKind".to_string(),
            description: "driver sent unknown error kind".to_string(),
        };

        let mapped = PriceEstimationError::from(error);
        assert!(matches!(
            mapped,
            PriceEstimationError::EstimatorInternal(err)
            if err.to_string() == "driver sent unknown error kind"
        ));
    }
}

#[async_trait::async_trait]
impl TradeFinding for ExternalTradeFinder {
    #[instrument(skip_all)]
    async fn get_quote(&self, query: &Query) -> Result<Quote, TradeError> {
        // The driver only has a single endpoint to compute trades so we can
        // simply reuse the same logic here.
        let trade = self.get_trade(query).await?;
        let gas_estimate = trade
            .gas_estimate()
            .context("no gas estimate")
            .map_err(TradeError::Other)?;
        Ok(Quote {
            out_amount: trade
                .out_amount(&PriceQuery {
                    sell_token: query.sell_token,
                    buy_token: query.buy_token,
                    kind: query.kind,
                    in_amount: query.in_amount,
                })
                .map_err(TradeError::Other)?,
            gas_estimate,
            solver: trade.solver(),
            supports_fast_path: trade.supports_fast_path(),
            quote_id: trade.quote_id(),
            execution: QuoteExecution {
                interactions: map_interactions_data(trade.interactions()),
                pre_interactions: map_interactions_data(trade.pre_interactions()),
                jit_orders: trade.jit_orders().cloned().collect(),
            },
        })
    }

    #[instrument(skip_all)]
    async fn get_trade(&self, query: &Query) -> Result<TradeKind, TradeError> {
        self.shared_query(query).await
    }
}

pub mod dto {
    use {
        alloy::primitives::{Address, U256},
        app_data::AppDataHash,
        bytes_hex::BytesHex,
        model::{
            order::{BuyTokenDestination, OrderKind, SellTokenSource},
            signature::SigningScheme,
        },
        number::serialization::HexOrDecimalU256,
        serde::{Deserialize, Serialize},
        serde_with::serde_as,
        std::collections::HashMap,
    };

    #[serde_as]
    #[derive(Clone, Debug, Serialize)]
    #[serde(rename_all = "camelCase")]
    pub struct Order {
        pub sell_token: Address,
        pub buy_token: Address,
        #[serde_as(as = "HexOrDecimalU256")]
        pub amount: U256,
        pub kind: OrderKind,
        pub deadline: chrono::DateTime<chrono::Utc>,
        #[serde(default, skip_serializing_if = "std::ops::Not::not")]
        pub enable_fast_path: bool,
        pub quote_id: i64,
    }

    #[serde_as]
    #[derive(Clone, Debug, Deserialize)]
    #[serde(untagged)]
    pub enum QuoteKind {
        Legacy(LegacyQuote),
        Regular(Quote),
    }

    #[serde_as]
    #[derive(Clone, Debug, Deserialize)]
    #[serde(rename_all = "camelCase")]
    pub struct LegacyQuote {
        #[serde_as(as = "HexOrDecimalU256")]
        pub amount: U256,
        pub interactions: Vec<Interaction>,
        pub solver: Address,
        pub gas: Option<u64>,
        #[serde(default)]
        pub tx_origin: Option<Address>,
        #[serde(default)]
        pub supports_fast_path: bool,
    }

    #[serde_as]
    #[derive(Clone, Debug, Deserialize)]
    #[serde(rename_all = "camelCase")]
    pub struct Quote {
        #[serde_as(as = "HashMap<_, HexOrDecimalU256>")]
        pub clearing_prices: HashMap<Address, U256>,
        #[serde(default)]
        pub pre_interactions: Vec<Interaction>,
        #[serde(default)]
        pub interactions: Vec<Interaction>,
        pub solver: Address,
        pub gas: Option<u64>,
        pub tx_origin: Option<Address>,
        #[serde(default)]
        pub jit_orders: Vec<JitOrder>,
        #[serde(default)]
        pub supports_fast_path: bool,
    }

    #[serde_as]
    #[derive(Clone, Debug, Deserialize)]
    #[serde(rename_all = "camelCase")]
    pub struct Interaction {
        pub target: Address,
        #[serde_as(as = "HexOrDecimalU256")]
        pub value: U256,
        #[serde_as(as = "BytesHex")]
        pub call_data: Vec<u8>,
    }

    #[serde_as]
    #[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize)]
    #[serde(rename_all = "camelCase")]
    pub struct JitOrder {
        pub buy_token: Address,
        pub sell_token: Address,
        #[serde_as(as = "HexOrDecimalU256")]
        pub sell_amount: U256,
        #[serde_as(as = "HexOrDecimalU256")]
        pub buy_amount: U256,
        #[serde_as(as = "HexOrDecimalU256")]
        pub executed_amount: U256,
        pub receiver: Address,
        pub valid_to: u32,
        pub app_data: AppDataHash,
        pub side: Side,
        pub partially_fillable: bool,
        pub sell_token_source: SellTokenSource,
        pub buy_token_destination: BuyTokenDestination,
        #[serde_as(as = "BytesHex")]
        pub signature: Vec<u8>,
        pub signing_scheme: SigningScheme,
    }

    #[serde_as]
    #[derive(Clone, Debug, Deserialize)]
    #[serde(rename_all = "camelCase")]
    pub struct Error {
        pub kind: String,
        pub description: String,
    }

    #[serde_as]
    #[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize)]
    #[serde(rename_all = "camelCase")]
    pub enum Side {
        Buy,
        Sell,
    }
}
