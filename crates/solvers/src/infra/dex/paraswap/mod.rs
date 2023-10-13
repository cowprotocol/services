use {
    crate::{
        domain::{auction, dex, eth},
        util,
    },
    ethereum_types::Address,
};

mod dto;

pub const DEFAULT_URL: &str = "https://apiv5.paraswap.io";

/// Bindings to the ParaSwap API.
pub struct ParaSwap {
    client: reqwest::Client,
    config: Config,
}

#[derive(Debug)]
pub struct Config {
    /// The base URL for the ParaSwap API.
    pub endpoint: reqwest::Url,

    /// The DEXs to exclude when using ParaSwap.
    pub exclude_dexs: Vec<String>,

    /// The solver address.
    pub address: Address,

    /// Our partner name.
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
        tokens: &auction::Tokens,
    ) -> Result<dex::Swap, Error> {
        let price = self.price(order, tokens).await?;
        let transaction = self.transaction(&price, order, tokens, slippage).await?;
        Ok(dex::Swap {
            call: dex::Call {
                to: eth::ContractAddress(transaction.to),
                calldata: transaction.data,
            },
            input: eth::Asset {
                token: order.sell,
                amount: price.src_amount,
            },
            output: eth::Asset {
                token: order.buy,
                amount: price.dest_amount,
            },
            allowance: dex::Allowance {
                spender: eth::ContractAddress(price.token_transfer_proxy),
                amount: dex::Amount::new(price.src_amount),
            },
            gas: eth::Gas(price.gas_cost),
        })
    }

    /// Make a request to the `/prices` endpoint.
    async fn price(
        &self,
        order: &dex::Order,
        tokens: &auction::Tokens,
    ) -> Result<dto::Price, Error> {
        let price = util::http::roundtrip!(
            <dto::Price, dto::Error>;
            self.client
                .get(util::url::join(&self.config.endpoint, "prices"))
                .query(&dto::PriceQuery::new(&self.config, order, tokens)?)
        )
        .await?;
        Ok(price)
    }

    /// Make a request to the `/transactions` endpoint.
    async fn transaction(
        &self,
        price: &dto::Price,
        order: &dex::Order,
        tokens: &auction::Tokens,
        slippage: &dex::Slippage,
    ) -> Result<dto::Transaction, Error> {
        let body = dto::TransactionBody::new(price, &self.config, order, tokens, slippage)?;
        let transaction = util::http::roundtrip!(
            <dto::Transaction, dto::Error>;
            self.client
                .post(util::url::join(
                    &self.config.endpoint,
                    "transactions/1?ignoreChecks=true",
                ))
                .json(&body)
        )
        .await?;
        Ok(transaction)
    }
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("no swap could be found")]
    NotFound,
    #[error("decimals are missing for the swapped tokens")]
    MissingDecimals,
    #[error("api error {0}")]
    Api(String),
    #[error(transparent)]
    Http(util::http::Error),
}

impl From<util::http::RoundtripError<dto::Error>> for Error {
    fn from(err: util::http::RoundtripError<dto::Error>) -> Self {
        match err {
            util::http::RoundtripError::Http(err) => Self::Http(err),
            util::http::RoundtripError::Api(err) => match err.error.as_str() {
                "ESTIMATED_LOSS_GREATER_THAN_MAX_IMPACT"
                | "No routes found with enough liquidity"
                | "Too much slippage on quote, please try again" => Self::NotFound,
                _ => Self::Api(err.error),
            },
        }
    }
}
