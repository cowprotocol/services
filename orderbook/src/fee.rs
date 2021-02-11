use std::collections::HashMap;

use anyhow::Result;
use chrono::{DateTime, Duration, Utc};
use primitive_types::{H160, U256};
use std::sync::Mutex;

use crate::price_estimate::PriceEstimating;
use gas_estimation::GasPriceEstimating;

type Measurement = (U256, DateTime<Utc>);

pub struct MinFeeCalculator {
    price_estimator: Box<dyn PriceEstimating>,
    gas_estimator: Box<dyn GasPriceEstimating>,
    native_token: H160,
    // TODO persist past measurements to shared storage
    measurements: Mutex<HashMap<H160, Vec<Measurement>>>,
    now: Box<dyn Fn() -> DateTime<Utc>>,
}

const GAS_PER_ORDER: f64 = 100_000.0;
const STANDARD_VALIDITY_FOR_FEE_IN_SEC: i64 = 60;

impl MinFeeCalculator {
    pub fn new(
        price_estimator: Box<dyn PriceEstimating>,
        gas_estimator: Box<dyn GasPriceEstimating>,
        native_token: H160,
    ) -> Self {
        Self {
            price_estimator,
            gas_estimator,
            native_token,
            measurements: Default::default(),
            now: Box::new(Utc::now),
        }
    }
}

impl MinFeeCalculator {
    // Returns the minimum amount of fee required to accept an order selling the specified token
    // and an expiry date for the estimate.
    pub async fn min_fee(&self, token: H160) -> Result<Measurement> {
        let gas_price = self.gas_estimator.estimate().await?;
        let token_price = self
            .price_estimator
            .estimate_price(token, self.native_token)
            .await?;

        let min_fee = U256::from_f64_lossy(gas_price * token_price * GAS_PER_ORDER);
        let valid_until = (self.now)() + Duration::seconds(STANDARD_VALIDITY_FOR_FEE_IN_SEC);
        let result = (min_fee, valid_until);

        self.measurements
            .lock()
            .expect("Thread holding mutex panicked")
            .entry(token)
            .or_default()
            .push(result);
        Ok(result)
    }

    // Returns true if the fee satisfies a previous not yet expired estimate, or the fee is high enough given the current estimate.
    pub async fn is_valid_fee(&self, token: H160, fee: U256) -> bool {
        if let Some(measurements) = self
            .measurements
            .lock()
            .expect("Thread holding mutex panicked")
            .get_mut(&token)
        {
            measurements.retain(|(_, expiry_date)| expiry_date >= &(self.now)());
            if measurements
                .iter()
                .any(|(suggested_fee, _)| &fee >= suggested_fee)
            {
                return true;
            }
        }
        if let Ok((current_fee, _)) = self.min_fee(token).await {
            return fee >= current_fee;
        }
        false
    }
}

#[cfg(test)]
mod tests {
    use chrono::Duration;
    use std::cell::RefCell;
    use std::rc::Rc;
    use std::sync::Arc;

    use super::*;

    struct FakePriceEstimator(f64);
    #[async_trait::async_trait]
    impl PriceEstimating for FakePriceEstimator {
        async fn estimate_price(&self, _: H160, _: H160) -> Result<f64> {
            Ok(self.0)
        }
    }

    struct FakeGasEstimator(Arc<Mutex<f64>>);
    #[async_trait::async_trait]
    impl GasPriceEstimating for FakeGasEstimator {
        async fn estimate_with_limits(&self, _: f64, _: std::time::Duration) -> Result<f64> {
            Ok(*self.0.lock().unwrap())
        }
    }

    impl MinFeeCalculator {
        fn new_for_test(
            gas_estimator: Box<dyn GasPriceEstimating>,
            price_estimator: Box<dyn PriceEstimating>,
            now: Box<dyn Fn() -> DateTime<Utc>>,
        ) -> Self {
            Self {
                gas_estimator,
                price_estimator,
                native_token: Default::default(),
                measurements: Default::default(),
                now,
            }
        }
    }

    #[tokio::test]
    async fn accepts_min_fee_if_validated_before_expiry() {
        let gas_price = Arc::new(Mutex::new(100.0));
        let time = Rc::new(RefCell::new(Utc::now()));

        let gas_estimator = Box::new(FakeGasEstimator(gas_price.clone()));
        let price_estimator = Box::new(FakePriceEstimator(1.0));
        let time_copy = time.clone();
        let now = move || *time_copy.borrow();

        let fee_estimator =
            MinFeeCalculator::new_for_test(gas_estimator, price_estimator, Box::new(now));

        let token = H160::from_low_u64_be(1);
        let (fee, expiry) = fee_estimator.min_fee(token).await.unwrap();

        // Gas price increase after measurement
        *gas_price.lock().unwrap() *= 2.0;

        // fee is valid before expiry
        time.replace(expiry - Duration::seconds(10));
        assert!(fee_estimator.is_valid_fee(token, fee).await);

        // fee is invalid after expiry
        time.replace(expiry + Duration::seconds(10));
        assert_eq!(fee_estimator.is_valid_fee(token, fee).await, false);
    }

    #[tokio::test]
    async fn accepts_fee_if_higher_than_current_min_fee() {
        let gas_price = Arc::new(Mutex::new(100.0));

        let gas_estimator = Box::new(FakeGasEstimator(gas_price.clone()));
        let price_estimator = Box::new(FakePriceEstimator(1.0));

        let fee_estimator =
            MinFeeCalculator::new_for_test(gas_estimator, price_estimator, Box::new(Utc::now));

        let token = H160::from_low_u64_be(1);
        let (fee, _) = fee_estimator.min_fee(token).await.unwrap();

        let lower_fee = fee - U256::one();

        // slightly lower fee is not valid
        assert_eq!(fee_estimator.is_valid_fee(token, lower_fee).await, false);

        // Gas price reduces, and slightly lower fee is now valid
        *gas_price.lock().unwrap() /= 2.0;
        assert!(fee_estimator.is_valid_fee(token, lower_fee).await);
    }
}
