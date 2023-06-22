//! Core logic of the prices service. TODO Write more about SOLID and what
//! constitutes "core logic".

// TODO Remove this ASAP
#![allow(dead_code)]

use futures::future::join_all;

pub mod estimator;
pub mod eth;

pub use estimator::Estimator;

// TODO Seriously thinking about renaming this to Swap and maybe having
// Swap.estimate
/// A price estimate query. Specifies how much of one token should be converted
/// to another token.
#[derive(Debug, Clone, Copy)]
pub struct Query {
    /// The token to swap from.
    pub from: FromToken,
    /// The token to swap into.
    pub to: ToToken,
    /// The amount to swap.
    pub amount: FromAmount,
}

/// The final estimate returned to the end user. Currently, this is simply the
/// median value of the [`estimator::Estimate`]s returned by the [`Estimator`]s.
#[derive(Debug, Clone, Copy)]
pub struct Estimate {
    /// The amount of [`FromToken`] that the user pays.
    pub from: FromAmount,
    /// The amount of [`ToToken`] that the user receives.
    pub to: ToAmount,
    /// The amount of [`ToToken`] paid in fees.
    pub fee: ToAmount,
}

/// Estimate the price of a token swap.
pub async fn estimate(
    query: Query,
    deadline: Deadline,
    estimators: &[Box<dyn Estimator>],
) -> Result<Estimate, Error> {
    // Fetch the median estimate for the query.
    let estimate = median_estimate(query, deadline, estimators).await?;

    // Convert the fee from ETH to [`ToToken`].
    let fee = median_estimate(
        Query {
            from: eth::ETH_TOKEN.into(),
            to: query.to,
            amount: estimate.gas.0.into(),
        },
        deadline,
        estimators,
    )
    .await?;

    Ok(Estimate {
        from: query.amount,
        to: estimate.to,
        fee: fee.to,
    })
}

/// Fetch the median estimate from the estimators.
async fn median_estimate(
    query: Query,
    deadline: Deadline,
    estimators: &[Box<dyn Estimator>],
) -> Result<estimator::Estimate, Error> {
    // Fetch all estimates from the estimators.
    let mut estimates: Vec<_> = join_all(
        estimators
            .iter()
            .map(|estimator| estimator.estimate(query, deadline)),
    )
    .await
    .into_iter()
    .filter_map(|estimate| {
        // TODO Observe the error
        estimate.ok()
    })
    .collect();

    // Pick the median estimate.
    estimates.sort_by_key(|estimate| estimate.to);
    estimates.get(estimates.len() / 2).copied().ok_or(Error)
}

/// The token to convert from.
#[derive(Debug, Clone, Copy)]
pub struct FromToken(eth::TokenAddress);

impl From<FromToken> for eth::H160 {
    fn from(value: FromToken) -> Self {
        value.0 .0
    }
}

impl From<eth::H160> for FromToken {
    fn from(value: eth::H160) -> Self {
        Self(eth::TokenAddress(value))
    }
}

impl From<eth::TokenAddress> for FromToken {
    fn from(value: eth::TokenAddress) -> Self {
        Self(value)
    }
}

/// The token to convert into.
#[derive(Debug, Clone, Copy)]
pub struct ToToken(eth::TokenAddress);

impl From<ToToken> for eth::H160 {
    fn from(value: ToToken) -> Self {
        value.0 .0
    }
}

impl From<eth::H160> for ToToken {
    fn from(value: eth::H160) -> Self {
        Self(eth::TokenAddress(value))
    }
}

/// Amount of [`FromToken`].
#[derive(Debug, Clone, Copy)]
pub struct FromAmount(eth::U256);

impl From<FromAmount> for eth::U256 {
    fn from(value: FromAmount) -> Self {
        value.0
    }
}

impl From<eth::U256> for FromAmount {
    fn from(value: eth::U256) -> Self {
        Self(value)
    }
}

/// Amount of [`ToToken`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct ToAmount(eth::U256);

impl From<ToAmount> for eth::U256 {
    fn from(value: ToAmount) -> Self {
        value.0
    }
}

impl From<eth::U256> for ToAmount {
    fn from(value: eth::U256) -> Self {
        Self(value)
    }
}

/// The estimation deadline.
#[derive(Debug, Clone, Copy)]
pub struct Deadline(pub std::time::Duration);

impl From<Deadline> for std::time::Duration {
    fn from(value: Deadline) -> Self {
        value.0
    }
}

#[derive(Debug, thiserror::Error)]
#[error("estimation failed")]
pub struct Error;
