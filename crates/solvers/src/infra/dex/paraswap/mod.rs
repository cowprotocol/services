use {
    crate::domain::{auction::Auction, dex, eth},
    ethereum_types::Address,
};

mod dto;

/// Bindings to the ParaSwap API.
pub struct ParaSwap {
    client: reqwest::Client,
    config: Config,
}

#[derive(Debug)]
pub struct Config {
    /// The base URL for the ParaSwap API.
    pub endpoint: reqwest::Url,

    // TODO Document this
    pub exclude_dexs: Vec<String>,

    /// The solver address.
    pub address: Address,

    // TODO Document this
    pub partner: String,
}

impl ParaSwap {
    pub fn new(config: Config) -> Self {
        Self {
            client: reqwest::Client::new(),
            config,
        }
    }

    pub async fn swap(
        &self,
        order: &dex::Order,
        slippage: &dex::Slippage,
        auction: &Auction,
    ) -> Result<dex::Swap, Error> {
        // TODO Move these two blocks into price and transaction methods and document
        // them

        let request = self
            .client
            .get(self.config.endpoint.join("prices").unwrap())
            .query(&dto::PriceQuery::new(&self.config, order, auction))
            .build()?;
        tracing::trace!("Querying ParaSwap price API: {request:?}");
        let response = self.client.execute(request).await?;
        let status = response.status();
        let text = response.text().await?;
        tracing::trace!(%status, %text, "Response from ParaSwap price API");
        let price = serde_json::from_str::<dto::Response<dto::Price>>(&text)?.into_result()?;

        let request = self
            .client
            .get(format!(
                "${}/transactions/1?ignoreChecks=true",
                self.config.endpoint
            ))
            .query(&dto::TransactionBody::new(
                &price,
                &self.config,
                order,
                auction,
                slippage,
            ))
            .build()?;
        tracing::trace!("Querying ParaSwap transaction API: {request:?}");
        let response = self.client.execute(request).await?;
        let status = response.status();
        let text = response.text().await?;
        tracing::trace!(%status, %text, "Response from ParaSwap transaction API");
        let transaction =
            serde_json::from_str::<dto::Response<dto::Transaction>>(&text)?.into_result()?;

        Ok(dex::Swap {
            call: dex::Call {
                to: eth::ContractAddress(transaction.to),
                calldata: transaction.data,
            },
            input: eth::Asset {
                token: order.sell,
                amount: price.src_amount(),
            },
            output: eth::Asset {
                token: order.buy,
                amount: price.dest_amount(),
            },
            // TODO Is this allowance correct?
            allowance: dex::Allowance {
                spender: eth::ContractAddress(transaction.to),
                amount: dex::Amount::new(price.src_amount()),
            },
            gas: eth::Gas(transaction.gas),
        })
    }
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("no swap could be found")]
    NotFound,
    #[error("api error {0}")]
    Api(String),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
    #[error(transparent)]
    Http(#[from] reqwest::Error),
}

impl From<dto::Error> for Error {
    fn from(value: dto::Error) -> Self {
        // TODO Check the legacy code to figure out which errors should result in a
        // `NotFound`
        Self::Api(value.error)
    }
}
