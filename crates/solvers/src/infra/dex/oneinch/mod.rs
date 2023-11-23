use {
    crate::{
        domain::{auction, dex, eth},
        util,
    },
    ethereum_types::H160,
    std::sync::atomic::{self, AtomicU64},
    tracing::Instrument,
};

mod dto;

/// Bindings to the 1Inch swap API.
pub struct OneInch {
    client: reqwest::Client,
    endpoint: reqwest::Url,
    defaults: dto::Query,
    spender: eth::ContractAddress,
}

pub struct Config {
    /// The base URL for the 1Inch swap API.
    pub endpoint: Option<reqwest::Url>,

    /// The address of the Settlement contract.
    pub settlement: eth::ContractAddress,

    /// The 1Inch liquidity sources to consider when swapping.
    pub liquidity: Liquidity,

    /// The referrer address to use. Referrers are entitled to a portion of
    /// the positive slippage that 1Inch collects.
    pub referrer: Option<H160>,

    // The following configuration options tweak the complexity of the 1Inch
    // route that the API returns. Unfortunately, the exact definition (and
    // what each field actually controls) is fairly opaque and not well
    // documented.
    pub main_route_parts: Option<u32>,
    pub connector_tokens: Option<u32>,
    pub complexity_level: Option<u32>,
}

pub enum Liquidity {
    Any,
    Only(Vec<String>),
    Exclude(Vec<String>),
}

pub const DEFAULT_URL: &str = "https://api.1inch.io/v5.0/1/";

impl OneInch {
    pub async fn new(config: Config) -> Result<Self, Error> {
        let client = reqwest::Client::new();
        let endpoint = config
            .endpoint
            .unwrap_or_else(|| DEFAULT_URL.parse().unwrap());

        let protocols = match config.liquidity {
            Liquidity::Any => None,
            Liquidity::Only(protocols) => Some(protocols),
            Liquidity::Exclude(excluded) => {
                let liquidity = util::http::roundtrip!(
                    <dto::Liquidity, dto::Error>;
                    client.get(util::url::join(&endpoint, "liquidity-sources"))
                )
                .await?;

                let protocols = liquidity
                    .protocols
                    .into_iter()
                    .filter(|protocol| !excluded.contains(&protocol.id))
                    .map(|protocol| protocol.id)
                    .collect();
                Some(protocols)
            }
        };
        let defaults = dto::Query {
            from_address: config.settlement.0,
            protocols,
            referrer_address: Some(config.referrer.unwrap_or(config.settlement.0)),
            disable_estimate: Some(true),
            main_route_parts: config.main_route_parts,
            connector_tokens: config.connector_tokens,
            complexity_level: config.complexity_level,
            ..Default::default()
        };

        let spender = eth::ContractAddress(
            util::http::roundtrip!(
                <dto::Spender, dto::Error>;
                client.get(util::url::join(&endpoint, "approve/spender"))
            )
            .await?
            .address,
        );

        Ok(Self {
            client,
            endpoint,
            defaults,
            spender,
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
            .with_domain(order, slippage, gas_price)
            .ok_or(Error::OrderNotSupported)?;
        let swap = {
            // Set up a tracing span to make debugging of API requests easier.
            // Historically, debugging API requests to external DEXs was a bit
            // of a headache.
            static ID: AtomicU64 = AtomicU64::new(0);
            let id = ID.fetch_add(1, atomic::Ordering::Relaxed);
            self.quote(&query)
                .instrument(tracing::trace_span!("quote", id = %id))
                .await?
        };

        Ok(dex::Swap {
            call: dex::Call {
                to: eth::ContractAddress(swap.tx.to),
                calldata: swap.tx.data,
            },
            input: eth::Asset {
                token: order.sell,
                amount: swap.from_token_amount,
            },
            output: eth::Asset {
                token: order.buy,
                amount: swap.to_token_amount,
            },
            allowance: dex::Allowance {
                spender: self.spender,
                amount: dex::Amount::new(swap.from_token_amount),
            },
            gas: eth::Gas(swap.tx.gas.into()),
        })
    }

    async fn quote(&self, query: &dto::Query) -> Result<dto::Swap, Error> {
        let swap = util::http::roundtrip!(
            <dto::Swap, dto::Error>;
            self.client
                .get(util::url::join(&self.endpoint, "swap"))
                .query(query)
        )
        .await?;

        Ok(swap)
    }
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("order type is not supported")]
    OrderNotSupported,
    #[error("no valid swap could be found")]
    NotFound,
    #[error("rate limited")]
    RateLimited,
    #[error("api error {code}: {description}")]
    Api { code: i32, description: String },
    #[error(transparent)]
    Http(util::http::Error),
}

impl From<util::http::RoundtripError<dto::Error>> for Error {
    fn from(err: util::http::RoundtripError<dto::Error>) -> Self {
        match err {
            util::http::RoundtripError::Http(http_err) => match http_err {
                util::http::Error::Status(status_code, _) if status_code.as_u16() == 429 => {
                    Self::RateLimited
                }
                other_err => Self::Http(other_err),
            },
            util::http::RoundtripError::Api(err) => {
                // Unfortunately, AFAIK these codes aren't documented anywhere. These
                // based on empirical observations of what the API has returned in the
                // past.
                match err.description.as_str() {
                    "insufficient liquidity" => Self::NotFound,
                    _ => Self::Api {
                        code: err.status_code,
                        description: err.description,
                    },
                }
            }
        }
    }
}
