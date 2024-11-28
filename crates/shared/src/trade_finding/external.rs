//! A trade finder that uses an external driver.

use {
    crate::{
        price_estimation::{PriceEstimationError, Query},
        request_sharing::{BoxRequestSharing, RequestSharing},
        trade_finding::{
            map_interactions_data,
            Interaction,
            Quote,
            Trade,
            TradeError,
            TradeFinding,
        },
    },
    anyhow::{anyhow, Context},
    ethrpc::block_stream::CurrentBlockWatcher,
    futures::FutureExt,
    reqwest::{header, Client},
    url::Url,
};

pub struct ExternalTradeFinder {
    /// URL to call to in the driver to get a quote with call data for a trade.
    quote_endpoint: Url,

    /// Utility to make sure no 2 identical requests are in-flight at the same
    /// time. Instead of issuing a duplicated request this awaits the
    /// response of the in-flight request.
    sharing: BoxRequestSharing<Query, Result<Trade, PriceEstimationError>>,

    /// Client to issue http requests with.
    client: Client,

    /// Stream to retrieve latest block information for block-dependent queries.
    block_stream: CurrentBlockWatcher,

    /// Timeout of the quote request to the driver.
    timeout: std::time::Duration,
}

impl ExternalTradeFinder {
    pub fn new(
        driver: Url,
        client: Client,
        block_stream: CurrentBlockWatcher,
        timeout: std::time::Duration,
    ) -> Self {
        Self {
            quote_endpoint: crate::url::join(&driver, "quote"),
            sharing: RequestSharing::labelled(format!("tradefinder_{}", driver)),
            client,
            block_stream,
            timeout,
        }
    }

    /// Queries the `/quote` endpoint of the configured driver and deserializes
    /// the result into a Quote or Trade.
    async fn shared_query(&self, query: &Query) -> Result<Trade, TradeError> {
        let fut = move |query: &Query| {
            let order = dto::Order {
                sell_token: query.sell_token,
                buy_token: query.buy_token,
                amount: query.in_amount.get(),
                kind: query.kind,
                deadline: chrono::Utc::now() + self.timeout,
            };
            let block_dependent = query.block_dependent;
            let id = observe::request_id::get_task_local_storage();
            let timeout = self.timeout;
            let client = self.client.clone();
            let quote_endpoint = self.quote_endpoint.clone();
            let block_hash = self.block_stream.borrow().hash;

            async move {
                let mut request = client
                    .get(quote_endpoint)
                    .query(&order)
                    .header(header::CONTENT_TYPE, "application/json")
                    .header(header::ACCEPT, "application/json");

                if block_dependent {
                    request = request.header("X-Current-Block-Hash", block_hash.to_string())
                }

                if let Some(id) = id {
                    request = request.header("X-REQUEST-ID", id);
                }

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
                let quote = serde_json::from_str::<dto::QuoteKind>(&text).map_err(|err| {
                    if let Ok(err) = serde_json::from_str::<dto::Error>(&text) {
                        PriceEstimationError::from(err)
                    } else {
                        PriceEstimationError::EstimatorInternal(anyhow!(err))
                    }
                })?;
                match quote {
                    dto::QuoteKind::Legacy(quote) => Ok(Trade::from(quote)),
                    dto::QuoteKind::Regular(_) => Err(PriceEstimationError::EstimatorInternal(
                        anyhow!("Quote with JIT orders is not currently supported"),
                    )),
                }
            }
            .boxed()
        };

        self.sharing
            .shared_or_else(query.clone(), fut)
            .await
            .map_err(TradeError::from)
    }
}

impl From<dto::LegacyQuote> for Trade {
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

impl From<dto::Error> for PriceEstimationError {
    fn from(value: dto::Error) -> Self {
        match value.kind.as_str() {
            "QuotingFailed" => Self::NoLiquidity,
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

#[async_trait::async_trait]
impl TradeFinding for ExternalTradeFinder {
    async fn get_quote(&self, query: &Query) -> Result<Quote, TradeError> {
        // The driver only has a single endpoint to compute trades so we can simply
        // reuse the same logic here.
        let trade = self.get_trade(query).await?;
        let gas_estimate = trade
            .gas_estimate
            .context("no gas estimate")
            .map_err(TradeError::Other)?;
        Ok(Quote {
            out_amount: trade.out_amount,
            gas_estimate,
            solver: trade.solver,
            interactions: map_interactions_data(&trade.interactions),
        })
    }

    async fn get_trade(&self, query: &Query) -> Result<Trade, TradeError> {
        self.shared_query(query).await
    }
}

mod dto {
    use {
        app_data::AppDataHash,
        bytes_hex::BytesHex,
        ethcontract::{H160, U256},
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
        pub sell_token: H160,
        pub buy_token: H160,
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
        #[allow(unused)]
        Regular(Quote),
    }

    #[serde_as]
    #[derive(Clone, Debug, Deserialize)]
    #[serde(rename_all = "camelCase")]
    pub struct LegacyQuote {
        #[serde_as(as = "HexOrDecimalU256")]
        pub amount: U256,
        pub interactions: Vec<Interaction>,
        pub solver: H160,
        pub gas: Option<u64>,
        #[serde(default)]
        pub tx_origin: Option<H160>,
    }

    #[serde_as]
    #[derive(Clone, Debug, Deserialize)]
    #[serde(rename_all = "camelCase")]
    #[allow(unused)]
    pub struct Quote {
        #[serde_as(as = "HashMap<_, HexOrDecimalU256>")]
        pub clearing_prices: HashMap<H160, U256>,
        #[serde(default)]
        pub pre_interactions: Vec<Interaction>,
        #[serde(default)]
        pub interactions: Vec<Interaction>,
        pub solver: H160,
        pub gas: Option<u64>,
        pub tx_origin: Option<H160>,
        #[serde(default)]
        pub jit_orders: Vec<JitOrder>,
    }

    #[serde_as]
    #[derive(Clone, Debug, Deserialize)]
    #[serde(rename_all = "camelCase")]
    pub struct Interaction {
        pub target: H160,
        #[serde_as(as = "HexOrDecimalU256")]
        pub value: U256,
        #[serde_as(as = "BytesHex")]
        pub call_data: Vec<u8>,
    }

    #[serde_as]
    #[derive(Clone, Debug, Eq, PartialEq, Deserialize)]
    #[serde(rename_all = "camelCase")]
    #[allow(unused)]
    pub struct JitOrder {
        pub buy_token: H160,
        pub sell_token: H160,
        #[serde_as(as = "HexOrDecimalU256")]
        pub sell_amount: U256,
        #[serde_as(as = "HexOrDecimalU256")]
        pub buy_amount: U256,
        #[serde_as(as = "HexOrDecimalU256")]
        pub executed_amount: U256,
        pub receiver: H160,
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
    #[derive(Clone, Debug, Eq, PartialEq, Deserialize)]
    #[serde(rename_all = "camelCase")]
    pub enum Side {
        Buy,
        Sell,
    }
}
