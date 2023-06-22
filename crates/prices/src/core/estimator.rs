use {
    super::{eth, Deadline, Query, ToAmount},
    futures::future::BoxFuture,
};

/// A price estimator. Given a [`Query`] (which specifies how much of the
/// [`crate::FromToken`] should be converted into the [`crate::ToToken`]), the
/// estimator returns an [`Estimate`] of how much of the [`crate::ToToken`] will
/// be received, and how much will be paid in gas fees.
pub trait Estimator {
    fn estimate(
        &self,
        query: Query,
        // The deadline is intentionally not part of the query, to make it more difficult for
        // estimator implementations to accidentally forget to use it.
        deadline: Deadline,
    ) -> BoxFuture<'_, Result<Estimate, Error>>;
}

/// An estimate returned by an [`Estimator`].
#[derive(Debug, Clone, Copy)]
pub struct Estimate {
    pub to: ToAmount,
    /// The gas cost of the swap.
    pub gas: eth::Gas,
}

/// Estimator error.
#[derive(Debug, thiserror::Error)]
#[error(transparent)]
pub struct Error(Box<dyn std::error::Error>);

impl Error {
    pub fn new<E>(err: E) -> Self
    where
        E: std::error::Error + 'static,
    {
        Self(Box::new(err))
    }
}
