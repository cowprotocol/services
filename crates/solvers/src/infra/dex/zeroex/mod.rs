use {
    crate::{
        domain::{auction, dex, eth, order},
        util,
    },
    ethereum_types::H160,
    std::sync::atomic::{self, AtomicU64},
    tracing::Instrument,
};

mod dto;

/// Bindings to the 0x swap API.
pub struct ZeroEx {
    client: reqwest::Client,
    endpoint: reqwest::Url,
    defaults: dto::Query,
}

pub struct Config {
    /// The base URL for the 0x swap API.
    pub endpoint: reqwest::Url,

    /// 0x provides a gated API for partners that requires authentication
    /// by specifying this as header in the HTTP request.
    pub api_key: String,

    /// The list of excluded liquidity sources. Liquidity from these sources
    /// will not be considered when solving.
    pub excluded_sources: Vec<String>,

    /// The affiliate address to use.
    ///
    /// This is used by 0x for tracking and analytic purposes.
    pub affiliate: H160,

    /// The address of the settlement contract.
    pub settlement: eth::ContractAddress,

    /// Wether or not to enable RFQ-T liquidity.
    pub enable_rfqt: bool,

    /// Whether or not to enable slippage protection.
    pub enable_slippage_protection: bool,
}

impl ZeroEx {
    pub fn new(config: Config) -> Result<Self, CreationError> {
        let client = {
            let mut key = reqwest::header::HeaderValue::from_str(&config.api_key)?;
            key.set_sensitive(true);

            let mut headers = reqwest::header::HeaderMap::new();
            headers.insert("0x-api-key", key);

            reqwest::Client::builder()
                .default_headers(headers)
                .build()?
        };
        let defaults = dto::Query {
            taker_address: Some(config.settlement.0),
            excluded_sources: config.excluded_sources,
            skip_validation: true,
            intent_on_filling: config.enable_rfqt,
            affiliate_address: config.affiliate,
            enable_slippage_protection: config.enable_slippage_protection,
            ..Default::default()
        };

        Ok(Self {
            client,
            endpoint: config.endpoint,
            defaults,
        })
    }

    pub async fn swap(
        &self,
        order: &dex::Order,
        slippage: &dex::Slippage,
        gas_price: auction::GasPrice,
    ) -> Result<dex::Swap, Error> {
        let query = self
            .defaults
            .clone()
            .with_domain(order, slippage, gas_price);
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

        let max_sell_amount = match order.side {
            order::Side::Buy => slippage.add(quote.sell_amount),
            order::Side::Sell => quote.sell_amount,
        };

        Ok(dex::Swap {
            call: dex::Call {
                to: eth::ContractAddress(quote.to),
                calldata: quote.data,
            },
            input: eth::Asset {
                token: order.sell,
                amount: quote.sell_amount,
            },
            output: eth::Asset {
                token: order.buy,
                amount: quote.buy_amount,
            },
            allowance: dex::Allowance {
                spender: quote
                    .allowance_target
                    .ok_or(Error::MissingSpender)
                    .map(eth::ContractAddress)?,
                amount: dex::Amount::new(max_sell_amount),
            },
            gas: eth::Gas(quote.estimated_gas),
        })
    }

    async fn quote(&self, query: &dto::Query) -> Result<dto::Quote, Error> {
        let quote = util::http::roundtrip!(
            <dto::Quote, dto::Error>;
            self.client
                .get(util::url::join(&self.endpoint, "quote"))
                .query(query)
        )
        .await?;
        Ok(quote)
    }
}

#[derive(Debug, thiserror::Error)]
pub enum CreationError {
    #[error(transparent)]
    Header(#[from] reqwest::header::InvalidHeaderValue),
    #[error(transparent)]
    Client(#[from] reqwest::Error),
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("unable to find a quote")]
    NotFound,
    #[error("quote does not specify an approval spender")]
    MissingSpender,
    #[error("rate limited")]
    RateLimited,
    #[error("api error code {code}: {reason}")]
    Api { code: i64, reason: String },
    #[error(transparent)]
    Http(util::http::Error),
}

impl From<util::http::RoundtripError<dto::Error>> for Error {
    fn from(err: util::http::RoundtripError<dto::Error>) -> Self {
        match err {
            util::http::RoundtripError::Http(err) => Self::Http(err),
            util::http::RoundtripError::Api(err) => {
                // Unfortunately, AFAIK these codes aren't documented anywhere. These
                // based on empirical observations of what the API has returned in the
                // past.
                match err.code {
                    100 => Self::NotFound,
                    429 => Self::RateLimited,
                    _ => Self::Api {
                        code: err.code,
                        reason: err.reason,
                    },
                }
            }
        }
    }
}
