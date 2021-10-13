use super::GAS_PRICE_REFRESH_INTERVAL;
use futures::{stream, Stream, StreamExt};
use gas_estimation::{EstimatedGasPrice, GasPriceEstimating};
use std::time::Duration;

// Create a never ending stream of gas prices based on checking the estimator in fixed intervals
// and enforcing the minimum increase. Errors are ignored.
pub fn gas_price_stream(
    time_limit: Duration,
    gas_price_cap: f64,
    gas_limit: f64,
    estimator: &dyn GasPriceEstimating,
    initial_gas_price: Option<EstimatedGasPrice>,
) -> impl Stream<Item = EstimatedGasPrice> + '_ {
    let stream = stream::unfold(true, move |first_call| async move {
        if first_call {
            if let Some(initial_gas_price) = initial_gas_price {
                return Some((Ok(initial_gas_price), false));
            }
        } else {
            tokio::time::sleep(GAS_PRICE_REFRESH_INTERVAL).await;
        }
        let estimate = estimator.estimate_with_limits(gas_limit, time_limit).await;
        Some((estimate, false))
    })
    .filter_map(|gas_price_result| async move {
        match gas_price_result {
            Ok(gas_price) => {
                tracing::debug!("estimated gas price {:?}", gas_price);
                Some(gas_price)
            }
            Err(err) => {
                tracing::error!("gas price estimation failed: {:?}", err);
                None
            }
        }
    });
    transaction_retry::gas_price_increase::enforce_minimum_increase_and_cap(gas_price_cap, stream)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    struct TestEstimator;

    #[async_trait::async_trait]
    impl GasPriceEstimating for TestEstimator {
        async fn estimate_with_limits(
            &self,
            _: f64,
            time_limit: Duration,
        ) -> anyhow::Result<EstimatedGasPrice> {
            Ok(EstimatedGasPrice {
                legacy: 20. - time_limit.as_secs_f64(),
                eip1559: None,
            })
        }
    }
}
