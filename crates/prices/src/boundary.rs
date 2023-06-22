use {
    crate::core,
    futures::{future::BoxFuture, FutureExt},
};

/// An estimator which simply delegates to the legacy code.
pub struct Estimator {
    inner: Box<dyn shared::price_estimation::PriceEstimating>,
    /// The name of the legacy estimator, used for debugging purposes.
    name: &'static str,
}

impl core::Estimator for Estimator {
    fn estimate(
        &self,
        swap: core::Swap,
        deadline: core::Deadline,
    ) -> BoxFuture<'_, Result<core::estimator::Estimate, core::estimator::Error>> {
        async move {
            tokio::time::timeout(
                deadline.into(),
                shared::price_estimation::single_estimate(
                    self.inner.as_ref(),
                    &shared::price_estimation::Query {
                        sell_token: swap.from.into(),
                        buy_token: swap.to.into(),
                        in_amount: swap.amount.into(),
                        kind: model::order::OrderKind::Sell,
                        verification: None,
                    },
                ),
            )
            .await
            .map_err(core::estimator::Error::new)?
            .map(|estimate| core::estimator::Estimate {
                amount: estimate.out_amount.into(),
                gas: core::eth::U256::from(estimate.gas).into(),
            })
            .map_err(core::estimator::Error::new)
        }
        .boxed()
    }
}

impl std::fmt::Debug for Estimator {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Estimator")
            .field("name", &self.name)
            .finish()
    }
}
