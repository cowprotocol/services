//! A trade finder that uses an external driver.

use {
    crate::{
        PriceEstimationError,
        Query,
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
    },
    anyhow::{Context, anyhow},
    ethrpc::block_stream::CurrentBlockWatcher,
    futures::FutureExt,
    observe::tracing::distributed::headers::tracing_headers,
    request_sharing::{BoxRequestSharing, RequestSharing},
    reqwest::{Client, header},
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

    /// Utility to make sure no 2 identical requests are in-flight at the same
    /// time. Instead of issuing a duplicated request this awaits the
    /// response of the in-flight request.
    sharing: BoxRequestSharing<Query, SharedTradeResponse>,

    /// Client to issue http requests with.
    client: Client,

    /// Stream to retrieve latest block information for block-dependent queries.
    block_stream: CurrentBlockWatcher,
}

impl ExternalTradeFinder {
    pub fn new(driver: Url, client: Client, block_stream: CurrentBlockWatcher) -> Self {
        Self {
            quote_endpoint: crate::utils::join_url(&driver, "quote"),
            sharing: RequestSharing::labelled(format!("tradefinder_{driver}")),
            client,
            block_stream,
        }
    }

    /// Queries the `/quote` endpoint of the configured driver and deserializes
    /// the result into a Quote or Trade.
    async fn shared_query(&self, query: &Query) -> Result<TradeKind, TradeError> {
        let fut = move |query: &Query| {
            let order = dto::Order {
                sell_token: query.sell_token,
                buy_token: query.buy_token,
                amount: query.in_amount.get(),
                kind: query.kind,
                deadline: chrono::Utc::now() + query.timeout,
            };
            let block_dependent = query.block_dependent;
            let id = observe::tracing::distributed::request_id::from_current_span();
            let client = self.client.clone();
            let quote_endpoint = self.quote_endpoint.clone();
            let block_hash = self.block_stream.borrow().hash;
            let timeout = query.timeout;

            async move {
                let mut request = client
                    .get(quote_endpoint)
                    .timeout(timeout)
                    .query(&order)
                    .headers(tracing_headers())
                    .header(header::CONTENT_TYPE, "application/json")
                    .header(header::ACCEPT, "application/json");

                if block_dependent {
                    request = request.header("X-Current-Block-Hash", block_hash.to_string())
                }

                if let Some(ref id) = id {
                    request = request.header("X-REQUEST-ID", id.clone());
                }

                let result = async {
                    let response = request
                        .timeout(timeout)
                        .send()
                        .await
                        .map_err(|err| PriceEstimationError::EstimatorInternal(anyhow!(err)))?;
                    if response.status() == 429 {
                        return Err(PriceEstimationError::RateLimited);
                    }
                    let text = response
                        .text()
                        .await
                        .map_err(|err| PriceEstimationError::EstimatorInternal(anyhow!(err)))?;
                    serde_json::from_str::<dto::QuoteKind>(&text)
                        .map(TradeKind::from)
                        .map_err(|err| {
                            if let Ok(err) = serde_json::from_str::<dto::Error>(&text) {
                                PriceEstimationError::from(err)
                            } else {
                                PriceEstimationError::EstimatorInternal(anyhow!(err))
                            }
                        })
                }
                .await;

                SharedTradeResponse {
                    result,
                    request_id: id,
                }
            }
            .boxed()
        };

        let shared = self.sharing.shared_or_else(query.clone(), fut);
        let response = shared.future.await;

        if shared.is_shared {
            tracing::debug!(
                original_request_id = ?response.request_id,
                "reusing in-flight quote request"
            );
        }

        response.result.map_err(TradeError::from)
    }
}

impl From<dto::QuoteKind> for TradeKind {
    fn from(quote: dto::QuoteKind) -> Self {
        match quote {
            dto::QuoteKind::Legacy(quote) => TradeKind::Legacy(quote.into()),
            dto::QuoteKind::Regular(quote) => TradeKind::Regular(quote.into()),
        }
    }
}

impl From<dto::LegacyQuote> for LegacyTrade {
    fn from(quote: dto::LegacyQuote) -> Self {
        Self {
            out_amount: quote.amount,
            gas_estimate: quote.gas,
            interactions: quote
                .interactions
                .into_iter()
                .map(|interaction| Interaction {
                    target: interaction.target,
                    value: interaction.value,
                    data: interaction.call_data,
                })
                .collect(),
            solver: quote.solver,
            tx_origin: quote.tx_origin,
        }
    }
}

impl From<dto::Quote> for Trade {
    fn from(quote: dto::Quote) -> Self {
        Self {
            clearing_prices: quote.clearing_prices,
            gas_estimate: quote.gas,
            pre_interactions: quote
                .pre_interactions
                .into_iter()
                .map(|interaction| Interaction {
                    target: interaction.target,
                    value: interaction.value,
                    data: interaction.call_data,
                })
                .collect(),
            interactions: quote
                .interactions
                .into_iter()
                .map(|interaction| Interaction {
                    target: interaction.target,
                    value: interaction.value,
                    data: interaction.call_data,
                })
                .collect(),
            solver: quote.solver,
            tx_origin: quote.tx_origin,
            jit_orders: quote.jit_orders,
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
        // The driver only has a single endpoint to compute trades so we can simply
        // reuse the same logic here.
        let trade = self.get_trade(query).await?;
        let gas_estimate = trade
            .gas_estimate()
            .context("no gas estimate")
            .map_err(TradeError::Other)?;
        Ok(Quote {
            out_amount: trade
                .out_amount(
                    &query.buy_token,
                    &query.sell_token,
                    &query.in_amount.get(),
                    &query.kind,
                )
                .map_err(TradeError::Other)?,
            gas_estimate,
            solver: trade.solver(),
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
