use {
    crate::domain::{auction, dex},
    ethrpc::block_stream::CurrentBlockWatcher,
    reqwest::RequestBuilder,
};

pub mod bitget;
pub mod okx;
pub mod simulator;

pub use self::simulator::Simulator;

/// A supported external DEX/DEX aggregator API.
pub enum Dex {
    Bitget(bitget::Bitget),
    Okx(Box<okx::Okx>),
}

impl Dex {
    /// Computes a swap (including calldata, estimated input and output amounts
    /// and the required allowance) for the specified order.
    ///
    /// These computed swaps can be used to generate single order solutions.
    pub async fn swap(
        &self,
        order: &dex::Order,
        slippage: &dex::Slippage,
        tokens: &auction::Tokens,
    ) -> Result<dex::Swap, Error> {
        let swap = match self {
            Dex::Bitget(bitget) => bitget.swap(order, slippage, tokens).await?,
            Dex::Okx(okx) => okx.swap(order, slippage).await?,
        };
        Ok(swap)
    }
}

/// A categorized error that occurred building a swap with an external DEX/DEX
/// aggregator.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("order type is not supported")]
    OrderNotSupported,
    #[error("no valid swap interaction could be found")]
    NotFound,
    #[error("invalid request")]
    BadRequest,
    #[error("rate limited")]
    RateLimited,
    #[error("unavailable for legal reasons, banned tokens or similar")]
    UnavailableForLegalReasons,
    #[error(transparent)]
    Other(Box<dyn std::error::Error + Send + Sync>),
}

/// A wrapper around [`reqwest::Client`] to pre-set commonly used headers
/// and other properties on each request.
pub(crate) struct Client {
    /// Client to send requests.
    client: reqwest::Client,

    /// Block stream to read the current block.
    block_stream: Option<CurrentBlockWatcher>,
}

impl Client {
    pub fn new(client: reqwest::Client, block_stream: Option<CurrentBlockWatcher>) -> Self {
        Self {
            client,
            block_stream,
        }
    }

    /// Prepares a request builder which already has additional headers set.
    pub fn request(&self, method: reqwest::Method, url: reqwest::Url) -> RequestBuilder {
        let request = self.client.request(method, url);
        if let Some(stream) = &self.block_stream {
            // Set this header to easily support caching in an egress proxy.
            request.header("X-CURRENT-BLOCK-HASH", stream.borrow().hash.to_string())
        } else {
            request
        }
    }
}

impl Error {
    /// for instrumentization purposes
    pub fn format_variant(&self) -> &'static str {
        match self {
            Self::OrderNotSupported => "OrderNotSupported",
            Self::NotFound => "NotFound",
            Self::BadRequest => "BadRequest",
            Self::RateLimited => "RateLimited",
            Self::UnavailableForLegalReasons => "UnavailableForLegalReasons",
            Self::Other(_) => "Other",
        }
    }
}

impl From<okx::Error> for Error {
    fn from(err: okx::Error) -> Self {
        match err {
            okx::Error::OrderNotSupported => Self::OrderNotSupported,
            okx::Error::NotFound => Self::NotFound,
            okx::Error::RateLimited => Self::RateLimited,
            _ => Self::Other(Box::new(err)),
        }
    }
}

impl From<bitget::Error> for Error {
    fn from(err: bitget::Error) -> Self {
        match err {
            bitget::Error::OrderNotSupported => Self::OrderNotSupported,
            bitget::Error::NotFound => Self::NotFound,
            bitget::Error::MissingDecimals => Self::BadRequest,
            bitget::Error::RateLimited => Self::RateLimited,
            _ => Self::Other(Box::new(err)),
        }
    }
}
