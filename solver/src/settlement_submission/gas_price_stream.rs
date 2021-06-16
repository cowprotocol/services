use super::GAS_PRICE_REFRESH_INTERVAL;
use futures::{stream, Stream, StreamExt};
use gas_estimation::GasPriceEstimating;

// Create a never ending stream of gas prices based on checking the estimator in fixed intervals
// and enforcing the minimum increase. Errors are ignored.
pub fn gas_price_stream(
    target_confirm_time: std::time::Instant,
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
            tokio::time::sleep(GAS_PRICE_REFRESH_INTERVAL).await;
        }
        let remaining_time = tokio::time::Instant::from_std(target_confirm_time)
            .saturating_duration_since(tokio::time::Instant::now());
        let estimate = estimator
            .estimate_with_limits(gas_limit, remaining_time)
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

#[cfg(test)]
mod tests {
    use super::*;
    use futures::stream::StreamExt;
    use std::time::Duration;
    use tokio::time;

    struct TestEstimator;

    #[async_trait::async_trait]
    impl GasPriceEstimating for TestEstimator {
        async fn estimate_with_limits(&self, _: f64, time_limit: Duration) -> anyhow::Result<f64> {
            Ok(20. - time_limit.as_secs_f64())
        }
    }

    #[tokio::test]
    async fn stream_uses_current_time() {
        time::pause();

        let estimator = TestEstimator;
        let stream = gas_price_stream(
            (tokio::time::Instant::now() + Duration::from_secs(20)).into_std(),
            f64::INFINITY,
            0.,
            &estimator,
            None,
        );
        futures::pin_mut!(stream);

        let next = stream.next().await.unwrap();
        assert_eq!(next as u32, 0);
        let next = stream.next().await.unwrap();
        assert_eq!(next as u32, 15);
        let next = stream.next().await.unwrap();
        assert_eq!(next as u32, 20);
    }
}
