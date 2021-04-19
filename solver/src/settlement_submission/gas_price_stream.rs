use super::GAS_PRICE_REFRESH_INTERVAL;
use futures::{stream, Stream, StreamExt};
use gas_estimation::GasPriceEstimating;
use std::time::Duration;

// Create a never ending stream of gas prices based on checking the estimator in fixed intervals
// and enforcing the minimum increase. Errors are ignored.
pub fn gas_price_stream(
    target_confirm_time: Duration,
    gas_price_cap: f64,
    gas_limit: f64,
    estimator: &dyn GasPriceEstimating,
    initial_gas_price: Option<f64>,
) -> impl Stream<Item = f64> + '_ {
    let stream = stream::unfold(true, move |first_call| async move {
        if first_call {
            if let Some(initial_gas_price) = initial_gas_price {
                return Some((Ok(initial_gas_price), false));
            }
        } else {
            tokio::time::delay_for(GAS_PRICE_REFRESH_INTERVAL).await;
        }
        let estimate = estimator
            .estimate_with_limits(gas_limit, target_confirm_time)
            .await;
        Some((estimate, false))
    })
    .filter_map(|gas_price_result| async move {
        match gas_price_result {
            Ok(gas_price) => {
                tracing::debug!("estimated gas price {}", gas_price);
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
