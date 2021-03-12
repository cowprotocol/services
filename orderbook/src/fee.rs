use std::collections::HashMap;

use anyhow::Result;
use chrono::{DateTime, Duration, Utc};
use model::order::OrderKind;
use primitive_types::{H160, U256};
use std::sync::Mutex;

use crate::database::Database;
use gas_estimation::GasPriceEstimating;
use shared::price_estimate::PriceEstimating;

type Measurement = (U256, DateTime<Utc>);

pub struct MinFeeCalculator {
    price_estimator: Box<dyn PriceEstimating>,
    gas_estimator: Box<dyn GasPriceEstimating>,
    native_token: H160,
    measurements: Box<dyn MinFeeStoring>,
    now: Box<dyn Fn() -> DateTime<Utc> + Send + Sync>,
}

#[async_trait::async_trait]
pub trait MinFeeStoring: Send + Sync {
    // Stores the given measurement. Returns an error if this fails
    async fn save_fee_measurement(
        &self,
        sell_token: H160,
        buy_token: Option<H160>,
        amount: Option<U256>,
        kind: Option<OrderKind>,
        expiry: DateTime<Utc>,
        min_fee: U256,
    ) -> Result<()>;

    // Return a vector of previously stored measurements for the given token that have an expiry >= min expiry
    // If buy_token or sell_amount is not specified, it will return the lowest estimate matching the values provided.
    async fn get_min_fee(
        &self,
        sell_token: H160,
        buy_token: Option<H160>,
        amount: Option<U256>,
        kind: Option<OrderKind>,
        min_expiry: DateTime<Utc>,
    ) -> Result<Option<U256>>;
}

const GAS_PER_ORDER: f64 = 100_000.0;

// We use a longer validity internally for persistence to avoid writing a value to storage on every request
// This way we can serve a previous estimate if the same token is queried again shortly after
const STANDARD_VALIDITY_FOR_FEE_IN_SEC: i64 = 60;
const PERSISTED_VALIDITY_FOR_FEE_IN_SEC: i64 = 120;

impl MinFeeCalculator {
    pub fn new(
        price_estimator: Box<dyn PriceEstimating>,
        gas_estimator: Box<dyn GasPriceEstimating>,
        native_token: H160,
        database: Database,
    ) -> Self {
        Self {
            price_estimator,
            gas_estimator,
            native_token,
            measurements: Box::new(database),
            now: Box::new(Utc::now),
        }
    }
}

impl MinFeeCalculator {
    // Returns the minimum amount of fee required to accept an order selling the specified token
    // and an expiry date for the estimate.
    // Returns an error if there is some estimation error and Ok(None) if no information about the given
    // token exists
    pub async fn min_fee(
        &self,
        sell_token: H160,
        buy_token: Option<H160>,
        amount: Option<U256>,
        kind: Option<OrderKind>,
    ) -> Result<Option<Measurement>> {
        let now = (self.now)();
        let official_valid_until = now + Duration::seconds(STANDARD_VALIDITY_FOR_FEE_IN_SEC);
        let internal_valid_until = now + Duration::seconds(PERSISTED_VALIDITY_FOR_FEE_IN_SEC);

        if let Ok(Some(past_fee)) = self
            .measurements
            .get_min_fee(sell_token, buy_token, amount, kind, official_valid_until)
            .await
        {
            return Ok(Some((past_fee, official_valid_until)));
        }

        let min_fee = match self.compute_min_fee(sell_token).await? {
            Some(fee) => fee,
            None => return Ok(None),
        };

        let _ = self
            .measurements
            .save_fee_measurement(
                sell_token,
                buy_token,
                amount,
                kind,
                internal_valid_until,
                min_fee,
            )
            .await;
        Ok(Some((min_fee, official_valid_until)))
    }

    async fn compute_min_fee(&self, token: H160) -> Result<Option<U256>> {
        let gas_price = self.gas_estimator.estimate().await?;
        let fee_in_eth = gas_price * GAS_PER_ORDER;
        let token_price = match self
            .price_estimator
            .estimate_price(
                token,
                self.native_token,
                U256::from_f64_lossy(fee_in_eth),
                model::order::OrderKind::Buy,
            )
            .await
        {
            Ok(price) => price,
            Err(err) => {
                tracing::warn!("Failed to estimate sell token price: {}", err);
                return Ok(None);
            }
        };

        Ok(Some(U256::from_f64_lossy(fee_in_eth * token_price)))
    }

    // Returns true if the fee satisfies a previous not yet expired estimate, or the fee is high enough given the current estimate.
    pub async fn is_valid_fee(&self, sell_token: H160, fee: U256) -> bool {
        if let Ok(Some(past_fee)) = self
            .measurements
            .get_min_fee(sell_token, None, None, None, (self.now)())
            .await
        {
            if fee >= past_fee {
                return true;
            }
        }
        if let Ok(Some(current_fee)) = self.compute_min_fee(sell_token).await {
            return fee >= current_fee;
        }
        false
    }
}

struct FeeMeasurement {
    buy_token: Option<H160>,
    amount: Option<U256>,
    kind: Option<OrderKind>,
    expiry: DateTime<Utc>,
    min_fee: U256,
}

#[derive(Default)]
struct InMemoryFeeStore(Mutex<HashMap<H160, Vec<FeeMeasurement>>>);
#[async_trait::async_trait]
impl MinFeeStoring for InMemoryFeeStore {
    async fn save_fee_measurement(
        &self,
        sell_token: H160,
        buy_token: Option<H160>,
        amount: Option<U256>,
        kind: Option<OrderKind>,
        expiry: DateTime<Utc>,
        min_fee: U256,
    ) -> Result<()> {
        self.0
            .lock()
            .expect("Thread holding Mutex panicked")
            .entry(sell_token)
            .or_default()
            .push(FeeMeasurement {
                buy_token,
                amount,
                kind,
                expiry,
                min_fee,
            });
        Ok(())
    }

    async fn get_min_fee(
        &self,
        sell_token: H160,
        buy_token: Option<H160>,
        amount: Option<U256>,
        kind: Option<OrderKind>,
        min_expiry: DateTime<Utc>,
    ) -> Result<Option<U256>> {
        let mut guard = self.0.lock().expect("Thread holding Mutex panicked");
        let measurements = guard.entry(sell_token).or_default();
        measurements.retain(|measurement| {
            if buy_token.is_some() && buy_token != measurement.buy_token {
                return false;
            }
            if amount.is_some() && amount != measurement.amount {
                return false;
            }
            if kind.is_some() && kind != measurement.kind {
                return false;
            }
            measurement.expiry >= min_expiry
        });
        Ok(measurements
            .iter()
            .map(|measurement| measurement.min_fee)
            .min())
    }
}

#[cfg(test)]
mod tests {
    use chrono::Duration;
    use model::order::OrderKind;
    use std::sync::Arc;

    use super::*;

    struct FakePriceEstimator(f64);
    #[async_trait::async_trait]
    impl PriceEstimating for FakePriceEstimator {
        async fn estimate_price(&self, _: H160, _: H160, _: U256, _: OrderKind) -> Result<f64> {
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
            now: Box<dyn Fn() -> DateTime<Utc> + Send + Sync>,
        ) -> Self {
            Self {
                gas_estimator,
                price_estimator,
                native_token: Default::default(),
                measurements: Box::new(InMemoryFeeStore::default()),
                now,
            }
        }
    }

    #[tokio::test]
    async fn accepts_min_fee_if_validated_before_expiry() {
        let gas_price = Arc::new(Mutex::new(100.0));
        let time = Arc::new(Mutex::new(Utc::now()));

        let gas_estimator = Box::new(FakeGasEstimator(gas_price.clone()));
        let price_estimator = Box::new(FakePriceEstimator(1.0));
        let time_copy = time.clone();
        let now = move || *time_copy.lock().unwrap();

        let fee_estimator =
            MinFeeCalculator::new_for_test(gas_estimator, price_estimator, Box::new(now));

        let token = H160::from_low_u64_be(1);
        let (fee, expiry) = fee_estimator
            .min_fee(token, None, None, None)
            .await
            .unwrap()
            .unwrap();

        // Gas price increase after measurement
        *gas_price.lock().unwrap() *= 2.0;

        // fee is valid before expiry
        *time.lock().unwrap() = expiry - Duration::seconds(10);
        assert!(fee_estimator.is_valid_fee(token, fee).await);

        // fee is invalid for some uncached token
        let token = H160::from_low_u64_be(2);
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
        let (fee, _) = fee_estimator
            .min_fee(token, None, None, None)
            .await
            .unwrap()
            .unwrap();

        let lower_fee = fee - U256::one();

        // slightly lower fee is not valid
        assert_eq!(fee_estimator.is_valid_fee(token, lower_fee).await, false);

        // Gas price reduces, and slightly lower fee is now valid
        *gas_price.lock().unwrap() /= 2.0;
        assert!(fee_estimator.is_valid_fee(token, lower_fee).await);
    }
}
