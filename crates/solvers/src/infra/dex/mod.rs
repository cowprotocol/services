use crate::domain::{auction, dex};

pub mod balancer;
pub mod oneinch;
pub mod zeroex;

/// A supported external DEX/DEX aggregator API.
pub enum Dex {
    Balancer(balancer::Sor),
    OneInch(oneinch::OneInch),
    ZeroEx(zeroex::ZeroEx),
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
        gas_price: auction::GasPrice,
    ) -> Result<dex::Swap, Error> {
        let swap = match self {
            Dex::Balancer(balancer) => balancer.swap(order, slippage, gas_price).await?,
            Dex::OneInch(oneinch) => oneinch.swap(order, slippage, gas_price).await?,
            Dex::ZeroEx(zeroex) => zeroex.swap(order, slippage, gas_price).await?,
        };
        Ok(swap)
    }
}

/// A categorised error that occurred building a swap with an external DEX/DEX
/// aggregator.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("order type is not supported")]
    OrderNotSupported,
    #[error("no valid swap interaction could be found")]
    NotFound,
    #[error(transparent)]
    Other(Box<dyn std::error::Error + Send + Sync + 'static>),
}

impl From<balancer::Error> for Error {
    fn from(err: balancer::Error) -> Self {
        match err {
            balancer::Error::NotFound => Self::NotFound,
            _ => Self::Other(Box::new(err)),
        }
    }
}

impl From<oneinch::Error> for Error {
    fn from(err: oneinch::Error) -> Self {
        match err {
            oneinch::Error::OrderNotSupported => Self::OrderNotSupported,
            oneinch::Error::NotFound => Self::NotFound,
            _ => Self::Other(Box::new(err)),
        }
    }
}

impl From<zeroex::Error> for Error {
    fn from(err: zeroex::Error) -> Self {
        match err {
            zeroex::Error::NotFound => Self::NotFound,
            _ => Self::Other(Box::new(err)),
        }
    }
}
