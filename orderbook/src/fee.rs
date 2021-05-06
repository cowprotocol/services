use std::collections::{HashMap, HashSet};

use anyhow::Result;
use chrono::{DateTime, Duration, Utc};
use model::order::OrderKind;
use primitive_types::{H160, U256};
use std::sync::{Arc, Mutex};
use thiserror::Error;

use crate::database::Database;
use gas_estimation::GasPriceEstimating;
use shared::price_estimate::PriceEstimating;

type Measurement = (U256, DateTime<Utc>);

pub struct MinFeeCalculator {
    price_estimator: Arc<dyn PriceEstimating>,
    gas_estimator: Box<dyn GasPriceEstimating>,
    native_token: H160,
    measurements: Box<dyn MinFeeStoring>,
    now: Box<dyn Fn() -> DateTime<Utc> + Send + Sync>,
    discount_factor: f64,
    unsupported_tokens: HashSet<H160>,
}

#[async_trait::async_trait]
pub trait MinFeeStoring: Send + Sync {
    // Stores the given measurement. Returns an error if this fails
    async fn save_fee_measurement(
        &self,
        sell_token: H160,
        buy_token: H160,
        amount: U256,
        kind: OrderKind,
        expiry: DateTime<Utc>,
        min_fee: U256,
    ) -> Result<()>;

    // Return a vector of previously stored measurements for the given token that have an expiry >= min expiry
    // If buy_token or sell_amount is not specified, it will return the lowest estimate matching the values provided.
    async fn get_min_fee(
        &self,
        sell_token: H160,
        buy_token: H160,
        amount: U256,
        kind: OrderKind,
        min_expiry: DateTime<Utc>,
    ) -> Result<Option<U256>>;
}

// We use a longer validity internally for persistence to avoid writing a value to storage on every request
// This way we can serve a previous estimate if the same token is queried again shortly after
const STANDARD_VALIDITY_FOR_FEE_IN_SEC: i64 = 60;
const PERSISTED_VALIDITY_FOR_FEE_IN_SEC: i64 = 120;

#[derive(Error, Debug)]
pub enum MinFeeCalculationError {
    // Represents a failure when no liquidity between sell and buy token via the native token can be found
    #[error("Token not found")]
    NotFound,

    // Represents a failure when one of the tokens involved is not supported by the system
    #[error("Token {0:?} not supported")]
    UnsupportedToken(H160),

    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

impl MinFeeCalculator {
    pub fn new(
        price_estimator: Arc<dyn PriceEstimating>,
        gas_estimator: Box<dyn GasPriceEstimating>,
        native_token: H160,
        database: Database,
        discount_factor: f64,
        unsupported_tokens: HashSet<H160>,
    ) -> Self {
        Self {
            price_estimator,
            gas_estimator,
            native_token,
            measurements: Box::new(database),
            now: Box::new(Utc::now),
            discount_factor,
            unsupported_tokens,
        }
    }

    // Returns the minimum amount of fee required to accept an order selling the specified order
    // and an expiry date for the estimate.
    // Returns an error if there is some estimation error and Ok(None) if no information about the given
    // token exists
    pub async fn min_fee(
        &self,
        sell_token: H160,
        buy_token: H160,
        amount: U256,
        kind: OrderKind,
    ) -> Result<Measurement, MinFeeCalculationError> {
        if self.unsupported_tokens.contains(&sell_token) {
            return Err(MinFeeCalculationError::UnsupportedToken(sell_token));
        }
        if self.unsupported_tokens.contains(&buy_token) {
            return Err(MinFeeCalculationError::UnsupportedToken(buy_token));
        }

        let now = (self.now)();
        let official_valid_until = now + Duration::seconds(STANDARD_VALIDITY_FOR_FEE_IN_SEC);
        let internal_valid_until = now + Duration::seconds(PERSISTED_VALIDITY_FOR_FEE_IN_SEC);

        if let Ok(Some(past_fee)) = self
            .measurements
            .get_min_fee(sell_token, buy_token, amount, kind, official_valid_until)
            .await
        {
            return Ok((past_fee, official_valid_until));
        }

        let min_fee = match self
            .compute_min_fee(sell_token, buy_token, amount, kind)
            .await?
        {
            Some(fee) => fee,
            None => return Err(MinFeeCalculationError::NotFound),
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
        Ok((min_fee, official_valid_until))
    }

    async fn compute_min_fee(
        &self,
        sell_token: H160,
        buy_token: H160,
        amount: U256,
        kind: OrderKind,
    ) -> Result<Option<U256>> {
        let gas_price = self.gas_estimator.estimate().await?;
        let gas_amount = match self
            .price_estimator
            .estimate_gas(sell_token, buy_token, amount, kind)
            .await
        {
            Ok(amount) => amount.to_f64_lossy() * self.discount_factor,
            Err(err) => {
                tracing::warn!("Failed to estimate gas amount: {}", err);
                return Ok(None);
            }
        };
        let fee_in_eth = gas_price * gas_amount;
        let token_price = match self
            .price_estimator
            .estimate_price_as_f64(
                sell_token,
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
    pub async fn is_valid_fee(
        &self,
        sell_token: H160,
        buy_token: H160,
        amount: U256,
        kind: OrderKind,
        fee: U256,
    ) -> bool {
        if let Ok(Some(past_fee)) = self
            .measurements
            .get_min_fee(sell_token, buy_token, amount, kind, (self.now)())
            .await
        {
            if fee >= past_fee {
                return true;
            }
        }
        if let Ok(Some(current_fee)) = self
            .compute_min_fee(sell_token, buy_token, amount, kind)
            .await
        {
            return fee >= current_fee;
        }
        false
    }
}

struct FeeMeasurement {
    buy_token: H160,
    amount: U256,
    kind: OrderKind,
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
        buy_token: H160,
        amount: U256,
        kind: OrderKind,
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
        buy_token: H160,
        amount: U256,
        kind: OrderKind,
        min_expiry: DateTime<Utc>,
    ) -> Result<Option<U256>> {
        let mut guard = self.0.lock().expect("Thread holding Mutex panicked");
        let measurements = guard.entry(sell_token).or_default();
        measurements.retain(|measurement| {
            if buy_token != measurement.buy_token {
                return false;
            }
            if amount != measurement.amount {
                return false;
            }
            if kind != measurement.kind {
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
    use maplit::hashset;
    use shared::gas_price_estimation::FakeGasPriceEstimator;
    use shared::price_estimate::mocks::FakePriceEstimator;
    use std::{collections::HashSet, sync::Arc};

    use super::*;

    impl MinFeeCalculator {
        fn new_for_test(
            gas_estimator: Box<dyn GasPriceEstimating>,
            price_estimator: Arc<dyn PriceEstimating>,
            now: Box<dyn Fn() -> DateTime<Utc> + Send + Sync>,
        ) -> Self {
            Self {
                gas_estimator,
                price_estimator,
                native_token: Default::default(),
                measurements: Box::new(InMemoryFeeStore::default()),
                now,
                discount_factor: 1.0,
                unsupported_tokens: HashSet::new(),
            }
        }
    }

    #[tokio::test]
    async fn accepts_min_fee_if_validated_before_expiry() {
        let gas_price = Arc::new(Mutex::new(100.0));
        let time = Arc::new(Mutex::new(Utc::now()));

        let gas_price_estimator = Box::new(FakeGasPriceEstimator(gas_price.clone()));
        let price_estimator = Arc::new(FakePriceEstimator(num::one()));
        let time_copy = time.clone();
        let now = move || *time_copy.lock().unwrap();

        let fee_estimator =
            MinFeeCalculator::new_for_test(gas_price_estimator, price_estimator, Box::new(now));

        let sell_token = H160::from_low_u64_be(1);
        let buy_token = H160::from_low_u64_be(2);
        let (fee, expiry) = fee_estimator
            .min_fee(sell_token, buy_token, 100.into(), OrderKind::Sell)
            .await
            .unwrap();

        // Gas price increase after measurement
        *gas_price.lock().unwrap() *= 2.0;

        // fee is valid before expiry
        *time.lock().unwrap() = expiry - Duration::seconds(10);
        assert!(
            fee_estimator
                .is_valid_fee(sell_token, buy_token, 100.into(), OrderKind::Sell, fee)
                .await
        );

        // fee is invalid for some uncached token
        let token = H160::from_low_u64_be(2);
        assert_eq!(
            fee_estimator
                .is_valid_fee(token, buy_token, 100.into(), OrderKind::Sell, fee)
                .await,
            false
        );
    }

    #[tokio::test]
    async fn accepts_fee_if_higher_than_current_min_fee() {
        let gas_price = Arc::new(Mutex::new(100.0));

        let gas_price_estimator = Box::new(FakeGasPriceEstimator(gas_price.clone()));
        let price_estimator = Arc::new(FakePriceEstimator(num::one()));

        let fee_estimator = MinFeeCalculator::new_for_test(
            gas_price_estimator,
            price_estimator,
            Box::new(Utc::now),
        );

        let sell_token = H160::from_low_u64_be(1);
        let buy_token = H160::from_low_u64_be(2);
        let (fee, _) = fee_estimator
            .min_fee(sell_token, buy_token, 100.into(), OrderKind::Sell)
            .await
            .unwrap();

        let lower_fee = fee - U256::one();

        // slightly lower fee is not valid
        assert_eq!(
            fee_estimator
                .is_valid_fee(
                    sell_token,
                    buy_token,
                    100.into(),
                    OrderKind::Sell,
                    lower_fee
                )
                .await,
            false
        );

        // Gas price reduces, and slightly lower fee is now valid
        *gas_price.lock().unwrap() /= 2.0;
        assert!(
            fee_estimator
                .is_valid_fee(
                    sell_token,
                    buy_token,
                    100.into(),
                    OrderKind::Sell,
                    lower_fee
                )
                .await
        );
    }

    #[tokio::test]
    async fn fails_for_unsupported_tokens() {
        let unsupported_token = H160::from_low_u64_be(1);
        let supported_token = H160::from_low_u64_be(2);

        let gas_price_estimator = Box::new(FakeGasPriceEstimator(Arc::new(Mutex::new(100.0))));
        let price_estimator = Arc::new(FakePriceEstimator(num::one()));
        let unsupported_tokens = hashset! {unsupported_token};

        let fee_estimator = MinFeeCalculator {
            price_estimator,
            gas_estimator: gas_price_estimator,
            native_token: Default::default(),
            measurements: Box::new(InMemoryFeeStore::default()),
            now: Box::new(Utc::now),
            discount_factor: 1.0,
            unsupported_tokens,
        };

        // Selling unsupported token
        assert!(matches!(
            fee_estimator
                .min_fee(
                    unsupported_token,
                    supported_token,
                    100.into(),
                    OrderKind::Sell)
                .await,
            Err(MinFeeCalculationError::UnsupportedToken(t)) if t == unsupported_token
        ));

        // Buying unsupported token
        assert!(matches!(
            fee_estimator
                .min_fee(
                    supported_token,
                    unsupported_token,
                    100.into(),
                    OrderKind::Sell
                )
                .await,
            Err(MinFeeCalculationError::UnsupportedToken(t)) if t == unsupported_token
        ));
    }
}
