pub mod baseline;
pub mod cached;
pub mod competition;
pub mod gas;
pub mod instrumented;
pub mod native;
pub mod native_price_cache;
pub mod oneinch;
pub mod paraswap;
pub mod priority;
pub mod quasimodo;
pub mod sanitized;
pub mod zeroex;

use crate::{bad_token::BadTokenDetecting, conversions::U256Ext};
use anyhow::Result;
use ethcontract::{H160, U256};
use model::order::OrderKind;
use num::BigRational;
use thiserror::Error;

#[derive(Copy, Clone, Debug, clap::ArgEnum)]
#[clap(rename_all = "verbatim")]
pub enum PriceEstimatorType {
    Baseline,
    Paraswap,
    ZeroEx,
    Quasimodo,
    OneInch,
}

impl PriceEstimatorType {
    /// Returns the name of this price estimator type.
    pub fn name(&self) -> String {
        format!("{:?}", self)
    }
}

#[derive(Error, Debug)]
pub enum PriceEstimationError {
    #[error("Token {0:?} not supported")]
    UnsupportedToken(H160),

    #[error("No liquidity")]
    NoLiquidity,

    #[error("Zero Amount")]
    ZeroAmount,

    #[error("Unsupported Order Type")]
    UnsupportedOrderType,

    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

impl Clone for PriceEstimationError {
    fn clone(&self) -> Self {
        match self {
            Self::UnsupportedToken(token) => Self::UnsupportedToken(*token),
            Self::NoLiquidity => Self::NoLiquidity,
            Self::ZeroAmount => Self::ZeroAmount,
            Self::UnsupportedOrderType => Self::UnsupportedOrderType,
            Self::Other(err) => Self::Other(crate::clone_anyhow_error(err)),
        }
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, Default)]
pub struct Query {
    pub sell_token: H160,
    pub buy_token: H160,
    /// For OrderKind::Sell amount is in sell_token and for OrderKind::Buy in buy_token.
    pub in_amount: U256,
    pub kind: OrderKind,
}

#[derive(Copy, Clone, Debug, Default, PartialEq)]
pub struct Estimate {
    pub out_amount: U256,
    /// full gas cost when settling this order alone on gp
    pub gas: U256,
}

impl Estimate {
    /// Returns (sell_amount, buy_amount).
    pub fn amounts(&self, query: &Query) -> (U256, U256) {
        match query.kind {
            OrderKind::Buy => (self.out_amount, query.in_amount),
            OrderKind::Sell => (query.in_amount, self.out_amount),
        }
    }

    /// The resulting price is how many units of sell_token needs to be sold for one unit of
    /// buy_token (sell_amount / buy_amount).
    pub fn price_in_sell_token_rational(&self, query: &Query) -> Option<BigRational> {
        let (sell_amount, buy_amount) = self.amounts(query);
        amounts_to_price(sell_amount, buy_amount)
    }

    /// The resulting price is how many units of sell_token needs to be sold for one unit of
    /// buy_token (sell_amount / buy_amount).
    pub fn price_in_sell_token_f64(&self, query: &Query) -> f64 {
        let (sell_amount, buy_amount) = self.amounts(query);
        sell_amount.to_f64_lossy() / buy_amount.to_f64_lossy()
    }
}

#[mockall::automock]
#[async_trait::async_trait]
pub trait PriceEstimating: Send + Sync {
    async fn estimate(&self, query: &Query) -> Result<Estimate, PriceEstimationError> {
        self.estimates(std::slice::from_ref(query))
            .await
            .into_iter()
            .next()
            .unwrap()
    }

    /// Returns one result for each query.
    async fn estimates(&self, queries: &[Query]) -> Vec<Result<Estimate, PriceEstimationError>>;
}

pub async fn ensure_token_supported(
    token: H160,
    bad_token_detector: &dyn BadTokenDetecting,
) -> Result<(), PriceEstimationError> {
    match bad_token_detector.detect(token).await {
        Ok(quality) => {
            if quality.is_good() {
                Ok(())
            } else {
                Err(PriceEstimationError::UnsupportedToken(token))
            }
        }
        Err(err) => Err(PriceEstimationError::Other(err)),
    }
}

pub fn amounts_to_price(sell_amount: U256, buy_amount: U256) -> Option<BigRational> {
    if buy_amount.is_zero() {
        return None;
    }
    Some(BigRational::new(
        sell_amount.to_big_int(),
        buy_amount.to_big_int(),
    ))
}

pub mod mocks {
    use super::*;
    use anyhow::anyhow;

    pub struct FakePriceEstimator(pub Estimate);
    #[async_trait::async_trait]
    impl PriceEstimating for FakePriceEstimator {
        async fn estimates(
            &self,
            queries: &[Query],
        ) -> Vec<Result<Estimate, PriceEstimationError>> {
            queries.iter().map(|_| Ok(self.0)).collect()
        }
    }

    pub struct FailingPriceEstimator();
    #[async_trait::async_trait]
    impl PriceEstimating for FailingPriceEstimator {
        async fn estimates(
            &self,
            queries: &[Query],
        ) -> Vec<Result<Estimate, PriceEstimationError>> {
            queries.iter().map(|_| Err(anyhow!("").into())).collect()
        }
    }
}
