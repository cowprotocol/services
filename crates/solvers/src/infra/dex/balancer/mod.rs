use {
    crate::domain::{auction, dex, eth, order},
    std::sync::atomic::{self, AtomicU64},
    tracing::Instrument,
};

mod dto;

/// Bindings to the Balancer Smart Order Router (SOR) API.
pub struct Sor {
    client: reqwest::Client,
    endpoint: reqwest::Url,
    settlement: eth::ContractAddress,
}

pub struct Config {
    /// The URL for the Balancer SOR API.
    pub endpoint: reqwest::Url,

    /// The address of the Settlement contract.
    pub settlement: eth::ContractAddress,
}

impl Sor {
    /// An approximate gas an individual Balancer swap uses.
    ///
    /// This value was determined heuristically using a Dune query that has been
    /// lost to time... See <https://github.com/cowprotocol/services/pull/171>.
    const GAS_PER_SWAP: u64 = 88_892;

    pub fn new(config: Config) -> Self {
        Self {
            client: reqwest::Client::new(),
            endpoint: config.endpoint,
            settlement: config.settlement,
        }
    }

    pub async fn swap(
        &self,
        order: &dex::Order,
        slippage: &dex::Slippage,
        gas_price: auction::GasPrice,
    ) -> Result<dex::Swap, Error> {
        let query = dto::Query::from_domain(order, gas_price, self.settlement);
        let quote = {
            // Set up a tracing span to make debugging of API requests easier.
            // Historically, debugging API requests to external DEXs was a bit
            // of a headache.
            static ID: AtomicU64 = AtomicU64::new(0);
            let id = ID.fetch_add(1, atomic::Ordering::Relaxed);
            self.quote(&query)
                .instrument(tracing::trace_span!("quote", id = %id))
                .await?
        };

        if quote.is_empty() {
            return Err(Error::NotFound);
        }

        let input = quote.price.sell_amount.hex;
        let output = quote.price.buy_amount.hex;

        let max_input = match order.side {
            order::Side::Buy => slippage.add(input),
            order::Side::Sell => input,
        };

        Ok(dex::Swap {
            call: dex::Call {
                to: eth::ContractAddress(quote.to),
                calldata: quote.data,
            },
            input: eth::Asset {
                token: eth::TokenAddress(order.sell.0),
                amount: input,
            },
            output: eth::Asset {
                token: eth::TokenAddress(order.buy.0),
                amount: output,
            },
            allowance: dex::Allowance {
                spender: eth::ContractAddress(quote.price.allowance_target),
                amount: dex::Amount::new(max_input),
            },
            // TODO: somehow get accurate gas estimate
            gas: eth::Gas(Sor::GAS_PER_SWAP.into()),
        })
    }

    async fn quote(&self, query: &dto::Query) -> Result<dto::Quote, Error> {
        let request = serde_json::to_string(&query)?;
        tracing::trace!(endpoint = %self.endpoint, %request, "quoting");
        let response = self
            .client
            .post(self.endpoint.clone())
            .header("content-type", "application/json")
            .body(request)
            .send()
            .await?
            .error_for_status()?
            .text()
            .await?;
        tracing::trace!(%response, "quoted");
        let quote = serde_json::from_str(&response)?;

        Ok(quote)
    }
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("no valid swap interaction could be found")]
    NotFound,
    #[error(transparent)]
    Json(#[from] serde_json::Error),
    #[error(transparent)]
    Http(#[from] reqwest::Error),
}
