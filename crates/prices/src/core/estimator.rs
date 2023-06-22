use {
    super::{eth, swap, Deadline, Swap},
    futures::future::BoxFuture,
};

/// An external token price estimator. Given a [`Swap`] (which specifies how
/// much of the [`crate::FromToken`] should be converted into the
/// [`crate::ToToken`]), the estimator returns an [`Estimate`] of how much of
/// the [`crate::ToToken`] will be received, and how much will be paid in gas
/// fees.
pub trait Estimator {
    fn estimate(&self, swap: Swap, deadline: Deadline) -> BoxFuture<'_, Result<Estimate, Error>>;
}

/// An estimate returned by an [`Estimator`].
#[derive(Debug, Clone, Copy)]
pub struct Estimate {
    pub to: swap::ToAmount,
    /// The gas cost of the swap.
    pub gas: eth::Gas,
}

/// Estimator error.
#[derive(Debug, thiserror::Error)]
#[error(transparent)]
pub struct Error(Box<dyn std::error::Error>);

impl Error {
    pub fn new(err: impl std::error::Error + 'static) -> Self {
        Self(Box::new(err))
    }
}
