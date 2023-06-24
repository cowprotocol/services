use {
    super::{eth, swap, Price, Swap},
    futures::future::{join_all, try_join, BoxFuture},
    std::str::FromStr,
};

/// The estimated outcome of a [`Swap`]. Currently, this is simply the median
/// value of the [`estimator::Estimate`]s returned by the [`Estimator`]s.
#[derive(Debug, Clone, Copy)]
pub struct Estimate {
    /// The amount of [`ToToken`] that the user receives.
    pub amount: swap::ToAmount,
    /// The amount of [`ToToken`] paid in gas fees.
    pub fee: swap::ToAmount,
}

// TODO Revisit this doc comment
/// An external token price estimator. Given a [`Swap`] (which specifies how
/// much of the [`crate::FromToken`] should be converted into the
/// [`crate::ToToken`]), the estimator returns an [`Estimate`] of how much of
/// the [`crate::ToToken`] will be received, and how much will be paid in gas
/// fees.
pub trait Estimator: std::fmt::Debug + Send + Sync + 'static {
    fn estimate(&self, swap: Swap) -> BoxFuture<'_, Result<(Price, eth::Gas), EstimatorError>>;
}

/// Estimate the outcome of a [`Swap`].
pub async fn estimate(
    swap: Swap,
    gas_price: eth::GasPrice,
    estimators: &[Box<dyn Estimator>],
) -> Result<Estimate, Error> {
    let ((swap_price, gas), (eth_price, _)) = try_join(
        // Fetch the estimate for the swap price.
        median_estimate(swap, estimators),
        // Fetch the estimate for ETH price, to convert the gas fees into [`ToToken`].
        median_estimate(
            Swap {
                // TODO Automatically wrap ETH, and specify the chain ID in the config
                // from: eth::ETH_TOKEN.into(),
                from: eth::H160::from_str("0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2")
                    .unwrap()
                    .into(),
                to: swap.to,
                amount: eth::U256::from(1000000000000000u64).into(),
            },
            estimators,
        ),
    )
    .await?;

    Ok(Estimate {
        amount: swap_price * swap.amount,
        fee: eth_price * swap::FromAmount::from(gas_price * gas),
    })
}

/// Fetch the median swap price and gas fee estimate from the [`Estimator`]s.
async fn median_estimate(
    swap: Swap,
    estimators: &[Box<dyn Estimator>],
) -> Result<(Price, eth::Gas), Error> {
    // Fetch all estimates from the estimators.
    let (mut prices, mut gas_fees): (Vec<_>, Vec<_>) =
        join_all(estimators.iter().map(|estimator| estimator.estimate(swap)))
            .await
            .into_iter()
            .filter_map(|estimate| {
                // TODO Observe the error
                estimate.ok()
            })
            .unzip();

    // Pick the median estimated price and gas fee.
    prices.sort_unstable_by_key(|&price| price * swap.amount);
    gas_fees.sort_unstable();
    Ok((
        prices.get(prices.len() / 2).copied().ok_or(Error)?,
        gas_fees.get(gas_fees.len() / 2).copied().ok_or(Error)?,
    ))
}

#[derive(Debug, thiserror::Error)]
#[error("estimation failed")]
pub struct Error;

#[derive(Debug, thiserror::Error)]
#[error(transparent)]
pub struct EstimatorError(Box<dyn std::error::Error + Send + Sync + 'static>);

impl EstimatorError {
    pub fn new(err: impl std::error::Error + Send + Sync + 'static) -> Self {
        Self(Box::new(err))
    }
}
