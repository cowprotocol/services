use {crate::core, futures::FutureExt};

/// An estimator which simply delegates to the legacy code.
pub struct Estimator(Box<dyn shared::price_estimation::PriceEstimating>);

impl core::Estimator for Estimator {
    fn estimate(
        &self,
        swap: core::Swap,
        deadline: core::Deadline,
    ) -> futures::future::BoxFuture<'_, Result<core::estimator::Estimate, core::estimator::Error>>
    {
        async move {
            tokio::time::timeout(
                deadline.into(),
                shared::price_estimation::single_estimate(
                    self.0.as_ref(),
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
                to: estimate.out_amount.into(),
                gas: core::eth::U256::from(estimate.gas).into(),
            })
            .map_err(core::estimator::Error::new)
        }
        .boxed()
    }
}
