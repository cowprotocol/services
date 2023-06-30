//! A trade finder that uses an external driver.

use {
    crate::{
        price_estimation::{PriceEstimationError, Query},
        request_sharing::RequestSharing,
        trade_finding::{Interaction, Quote, Trade, TradeError, TradeFinding},
    },
    anyhow::Context,
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
}

impl ExternalTradeFinder {
    #[allow(dead_code)]
    pub fn new(driver: Url, client: Client) -> Self {
        Self {
            quote_endpoint: crate::url::join(&driver, "quote"),
            sharing: Default::default(),
            client,
        }
    }

    /// Queries the `/quote` endpoint of the configured driver and deserializes
    /// the result into a Quote or Trade.
    async fn shared_query(&self, query: &Query) -> Result<Trade, TradeError> {
        let deadline = chrono::Utc::now() + Self::time_limit();
        let order = dto::Order {
            sell_token: query.sell_token,
            buy_token: query.buy_token,
            amount: query.in_amount,
            kind: query.kind,
            deadline,
        };

        let body = serde_json::to_string(&order).context("failed to encode body")?;
        let request = self
            .client
            .post(self.quote_endpoint.clone())
            .header(header::CONTENT_TYPE, "application/json")
            .header(header::ACCEPT, "application/json")
            .body(body);

        let future = async {
            let response = request.send().await.map_err(PriceEstimationError::from)?;
            if response.status() == 429 {
                return Err(PriceEstimationError::RateLimited);
            }
            let text = response.text().await.map_err(PriceEstimationError::from)?;
            serde_json::from_str::<dto::Quote>(&text)
                .map(Trade::from)
                .map_err(PriceEstimationError::from)
        };

        self.sharing
            .shared(query.clone(), future.boxed())
            .await
            .map_err(TradeError::from)
    }

    /// Returns the default time limit used for quoting with external co-located
    /// solvers.
    fn time_limit() -> chrono::Duration {
        chrono::Duration::seconds(5)
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
            gas_estimate: TRADE_GAS,
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

#[async_trait::async_trait]
impl TradeFinding for ExternalTradeFinder {
    async fn get_quote(&self, query: &Query) -> Result<Quote, TradeError> {
        // The driver only has a single endpoint to compute trades so we can simply
        // reuse the same logic here.
        let trade = self.get_trade(query).await?;
        Ok(Quote {
            out_amount: trade.out_amount,
            gas_estimate: trade.gas_estimate,
        })
    }

    async fn get_trade(&self, query: &Query) -> Result<Trade, TradeError> {
        self.shared_query(query).await
    }
}

mod dto {
    use {
        ethcontract::{H160, U256},
        model::{bytes_hex::BytesHex, order::OrderKind, u256_decimal::DecimalU256},
        serde::{Deserialize, Serialize},
        serde_with::serde_as,
    };

    #[serde_as]
    #[derive(Clone, Debug, Serialize)]
    #[serde(rename_all = "camelCase")]
    pub struct Order {
        pub sell_token: H160,
        pub buy_token: H160,
        #[serde_as(as = "DecimalU256")]
        pub amount: U256,
        pub kind: OrderKind,
        pub deadline: chrono::DateTime<chrono::Utc>,
    }

    #[serde_as]
    #[derive(Clone, Debug, Deserialize)]
    #[serde(rename_all = "camelCase")]
    pub struct Quote {
        #[serde_as(as = "DecimalU256")]
        pub amount: U256,
        pub interactions: Vec<Interaction>,
        pub solver: H160,
    }

    #[serde_as]
    #[derive(Clone, Debug, Deserialize)]
    #[serde(rename_all = "camelCase")]
    pub struct Interaction {
        pub target: H160,
        #[serde_as(as = "DecimalU256")]
        pub value: U256,
        #[serde_as(as = "BytesHex")]
        pub call_data: Vec<u8>,
    }
}
