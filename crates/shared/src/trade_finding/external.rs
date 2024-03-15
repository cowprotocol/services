//! A trade finder that uses an external driver.

use {
    crate::{
        price_estimation::{PriceEstimationError, Query},
        request_sharing::RequestSharing,
        trade_finding::{Interaction, Quote, Trade, TradeError, TradeFinding},
    },
    anyhow::anyhow,
    ethrpc::current_block::CurrentBlockStream,
    futures::{future::BoxFuture, FutureExt},
    reqwest::{header, Client},
    url::Url,
};

pub struct ExternalTradeFinder {
    /// URL to call to in the driver to get a quote with call data for a trade.
    quote_endpoint: Url,

    /// Utility to make sure no 2 identical requests are in-flight at the same
    /// time. Instead of issuing a duplicated request this awaits the
    /// response of the in-flight request.
    sharing: RequestSharing<Query, BoxFuture<'static, Result<Trade, PriceEstimationError>>>,

    /// Client to issue http requests with.
    client: Client,

    /// Stream to retrieve latest block information for block-dependent queries.
    block_stream: CurrentBlockStream,
}

impl ExternalTradeFinder {
    pub fn new(driver: Url, client: Client, block_stream: CurrentBlockStream) -> Self {
        Self {
            quote_endpoint: crate::url::join(&driver, "quote"),
            sharing: RequestSharing::labelled(format!("tradefinder_{}", driver)),
            client,
            block_stream,
        }
    }

    /// Queries the `/quote` endpoint of the configured driver and deserializes
    /// the result into a Quote or Trade.
    async fn shared_query(&self, query: &Query) -> Result<Trade, TradeError> {
        let deadline = chrono::Utc::now() + super::time_limit();
        let order = dto::Order {
            sell_token: query.sell_token,
            buy_token: query.buy_token,
            amount: query.in_amount.get(),
            kind: query.kind,
            deadline,
        };

        let mut request = self
            .client
            .get(self.quote_endpoint.clone())
            .query(&order)
            .header(header::CONTENT_TYPE, "application/json")
            .header(header::ACCEPT, "application/json");

        if query.block_dependent {
            request = request.header(
                "X-Current-Block-Hash",
                self.block_stream.borrow().hash.to_string(),
            )
        }

        if let Some(id) = observe::request_id::get_task_local_storage() {
            request = request.header("X-REQUEST-ID", id);
        }

        let future = async {
            let response = request
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
            serde_json::from_str::<dto::Quote>(&text)
                .map(Trade::from)
                .map_err(|err| {
                    if let Ok(err) = serde_json::from_str::<dto::Error>(&text) {
                        PriceEstimationError::from(err)
                    } else {
                        PriceEstimationError::EstimatorInternal(anyhow!(err))
                    }
                })
        };

        self.sharing
            .shared(query.clone(), future.boxed())
            .await
            .map_err(TradeError::from)
    }
}

impl From<dto::Quote> for Trade {
    fn from(quote: dto::Quote) -> Self {
        // TODO: We are currently deciding on whether or not we need indicative
        // fee estimates for indicative price estimates. If they are needed,
        // then this approximation is obviously inaccurate and should be
        // improved, and would likely involve including gas estimates in quote
        // responses from the driver or implementing this as a separate API.
        //
        // Value guessed from <https://dune.com/queries/1373225>
        const TRADE_GAS: u64 = 290_000;

        Self {
            out_amount: quote.amount,
            gas_estimate: quote.gas.unwrap_or(TRADE_GAS),
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

#[async_trait::async_trait]
impl TradeFinding for ExternalTradeFinder {
    async fn get_quote(&self, query: &Query) -> Result<Quote, TradeError> {
        // The driver only has a single endpoint to compute trades so we can simply
        // reuse the same logic here.
        let trade = self.get_trade(query).await?;
        Ok(Quote {
            out_amount: trade.out_amount,
            gas_estimate: trade.gas_estimate,
            solver: trade.solver,
        })
    }

    async fn get_trade(&self, query: &Query) -> Result<Trade, TradeError> {
        self.shared_query(query).await
    }
}

mod dto {
    use {
        ethcontract::{H160, U256},
        model::{bytes_hex::BytesHex, order::OrderKind},
        number::serialization::HexOrDecimalU256,
        serde::{Deserialize, Serialize},
        serde_with::serde_as,
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
    #[serde(rename_all = "camelCase")]
    pub struct Quote {
        #[serde_as(as = "HexOrDecimalU256")]
        pub amount: U256,
        pub interactions: Vec<Interaction>,
        pub solver: H160,
        pub gas: Option<u64>,
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
    #[derive(Clone, Debug, Deserialize)]
    #[serde(rename_all = "camelCase")]
    pub struct Error {
        pub kind: String,
        pub description: String,
    }
}
