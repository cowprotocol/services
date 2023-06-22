use {
    super::{estimator, eth, swap, Estimator, Swap},
    futures::future::join_all,
};

/// The estimated outcome of a [`Swap`]. Currently, this is simply the median
/// value of the [`estimator::Estimate`]s returned by the [`Estimator`]s.
#[derive(Debug, Clone, Copy)]
pub struct Estimate {
    /// The [`Swap`] that was estimated.
    pub swap: Swap,
    /// The amount of [`ToToken`] that the user receives.
    pub to: swap::ToAmount,
    /// The amount of [`ToToken`] paid in fees.
    pub fee: swap::ToAmount,
}

/// Estimate the outcome of a [`Swap`].
pub async fn estimate(
    swap: Swap,
    deadline: Deadline,
    estimators: &[Box<dyn Estimator>],
) -> Result<Estimate, Error> {
    // Fetch the estimate for the swap.
    let estimate = median_estimate(swap, deadline, estimators).await?;

    // Convert the fee from ETH to [`ToToken`].
    let fee = median_estimate(
        Swap {
            from: eth::ETH_TOKEN.into(),
            to: swap.to,
            amount: estimate.gas.0.into(),
        },
        deadline,
        estimators,
    )
    .await?;

    Ok(Estimate {
        swap,
        to: estimate.to,
        fee: fee.to,
    })
}

/// Fetch the median [`estimator::Estimate`] from the [`Estimator`]s.
async fn median_estimate(
    swap: Swap,
    deadline: Deadline,
    estimators: &[Box<dyn Estimator>],
) -> Result<estimator::Estimate, Error> {
    // Fetch all estimates from the estimators.
    let mut estimates: Vec<_> = join_all(
        estimators
            .iter()
            .map(|estimator| estimator.estimate(swap, deadline)),
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

/// The estimation deadline. Estimates coming after this deadline are dropped.
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
