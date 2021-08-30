use anyhow::Result;
use chrono::{DateTime, Duration, Utc};
use gas_estimation::GasPriceEstimating;
use model::order::{OrderKind, BUY_ETH_ADDRESS};
use primitive_types::{H160, U256};
use shared::{bad_token::BadTokenDetecting, price_estimate::PriceEstimating};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use thiserror::Error;

pub type Measurement = (U256, DateTime<Utc>);

pub type EthAwareMinFeeCalculator = EthAdapter<MinFeeCalculator>;

pub struct EthAdapter<T> {
    calculator: T,
    weth: H160,
}

pub struct MinFeeCalculator {
    price_estimator: Arc<dyn PriceEstimating>,
    gas_estimator: Arc<dyn GasPriceEstimating>,
    native_token: H160,
    measurements: Arc<dyn MinFeeStoring>,
    now: Box<dyn Fn() -> DateTime<Utc> + Send + Sync>,
    fee_factor: f64,
    bad_token_detector: Arc<dyn BadTokenDetecting>,
}

#[cfg_attr(test, mockall::automock)]
#[async_trait::async_trait]
pub trait MinFeeCalculating: Send + Sync {
    // Returns the minimum amount of fee required to accept an order selling the specified order
    // and an expiry date for the estimate.
    // Returns an error if there is some estimation error and Ok(None) if no information about the given
    // token exists
    async fn min_fee(
        &self,
        sell_token: H160,
        buy_token: Option<H160>,
        amount: Option<U256>,
        kind: Option<OrderKind>,
    ) -> Result<Measurement, MinFeeCalculationError>;

    // Returns true if the fee satisfies a previous not yet expired estimate, or the fee is high enough given the current estimate.
    async fn is_valid_fee(&self, sell_token: H160, fee: U256) -> bool;
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

fn normalize_buy_token(buy_token: H160, weth: H160) -> H160 {
    if buy_token == BUY_ETH_ADDRESS {
        weth
    } else {
        buy_token
    }
}

impl EthAwareMinFeeCalculator {
    pub fn new(
        price_estimator: Arc<dyn PriceEstimating>,
        gas_estimator: Arc<dyn GasPriceEstimating>,
        native_token: H160,
        measurements: Arc<dyn MinFeeStoring>,
        fee_factor: f64,
        bad_token_detector: Arc<dyn BadTokenDetecting>,
    ) -> Self {
        Self {
            calculator: MinFeeCalculator::new(
                price_estimator,
                gas_estimator,
                native_token,
                measurements,
                fee_factor,
                bad_token_detector,
            ),
            weth: native_token,
        }
    }
}

#[async_trait::async_trait]
impl<T> MinFeeCalculating for EthAdapter<T>
where
    T: MinFeeCalculating + Send + Sync,
{
    async fn min_fee(
        &self,
        sell_token: H160,
        buy_token: Option<H160>,
        amount: Option<U256>,
        kind: Option<OrderKind>,
    ) -> Result<Measurement, MinFeeCalculationError> {
        self.calculator
            .min_fee(
                sell_token,
                buy_token.map(|token| normalize_buy_token(token, self.weth)),
                amount,
                kind,
            )
            .await
    }

    async fn is_valid_fee(&self, sell_token: H160, fee: U256) -> bool {
        self.calculator.is_valid_fee(sell_token, fee).await
    }
}

impl MinFeeCalculator {
    fn new(
        price_estimator: Arc<dyn PriceEstimating>,
        gas_estimator: Arc<dyn GasPriceEstimating>,
        native_token: H160,
        measurements: Arc<dyn MinFeeStoring>,
        fee_factor: f64,
        bad_token_detector: Arc<dyn BadTokenDetecting>,
    ) -> Self {
        Self {
            price_estimator,
            gas_estimator,
            native_token,
            measurements,
            now: Box::new(Utc::now),
            fee_factor,
            bad_token_detector,
        }
    }

    async fn compute_min_fee(
        &self,
        sell_token: H160,
        buy_token: Option<H160>,
        amount: Option<U256>,
        kind: Option<OrderKind>,
    ) -> Result<Option<U256>> {
        let gas_price = self.gas_estimator.estimate().await?;
        let gas_amount =
            if let (Some(buy_token), Some(amount), Some(kind)) = (buy_token, amount, kind) {
                // We only apply the discount to the more sophisticated fee estimation, as the legacy one is already very favorable to the user in most cases
                match self
                    .price_estimator
                    .estimate_gas(sell_token, buy_token, amount, kind)
                    .await
                {
                    Ok(amount) => amount.to_f64_lossy() * self.fee_factor,
                    Err(err) => {
                        tracing::warn!("Failed to estimate gas amount: {}", err);
                        return Ok(None);
                    }
                }
            } else {
                GAS_PER_ORDER
            };
        let fee_in_eth = gas_price * gas_amount;
        let amount_to_estimate_price = U256::from_f64_lossy(fee_in_eth).max(U256::one());
        let token_price = match self
            .price_estimator
            .estimate_price_as_f64(
                sell_token,
                self.native_token,
                amount_to_estimate_price,
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

    async fn ensure_token_supported(&self, token: H160) -> Result<(), MinFeeCalculationError> {
        match self.bad_token_detector.detect(token).await {
            Ok(quality) => {
                if quality.is_good() {
                    Ok(())
                } else {
                    Err(MinFeeCalculationError::UnsupportedToken(token))
                }
            }
            Err(err) => Err(MinFeeCalculationError::Other(err)),
        }
    }
}

#[async_trait::async_trait]
impl MinFeeCalculating for MinFeeCalculator {
    // Returns the minimum amount of fee required to accept an order selling the specified order
    // and an expiry date for the estimate.
    // Returns an error if there is some estimation error and Ok(None) if no information about the given
    // token exists
    async fn min_fee(
        &self,
        sell_token: H160,
        buy_token: Option<H160>,
        amount: Option<U256>,
        kind: Option<OrderKind>,
    ) -> Result<Measurement, MinFeeCalculationError> {
        self.ensure_token_supported(sell_token).await?;
        if let Some(buy_token) = buy_token {
            self.ensure_token_supported(buy_token).await?;
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

    // Returns true if the fee satisfies a previous not yet expired estimate, or the fee is high enough given the current estimate.
    async fn is_valid_fee(&self, sell_token: H160, fee: U256) -> bool {
        if let Ok(Some(past_fee)) = self
            .measurements
            .get_min_fee(sell_token, None, None, None, (self.now)())
            .await
        {
            if fee >= past_fee {
                return true;
            }
        }
        if let Ok(Some(current_fee)) = self.compute_min_fee(sell_token, None, None, None).await {
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
    use chrono::{Duration, NaiveDateTime};
    use shared::{
        bad_token::list_based::ListBasedDetector, gas_price_estimation::FakeGasPriceEstimator,
        price_estimate::mocks::FakePriceEstimator,
    };
    use std::sync::Arc;

    use super::*;

    #[tokio::test]
    async fn eth_aware_min_fees() {
        let weth = H160([0x42; 20]);
        let token = H160([0x21; 20]);
        let mut calculator = MockMinFeeCalculating::default();
        calculator
            .expect_min_fee()
            .withf(move |&sell_token, &buy_token, &amount, &kind| {
                sell_token == token
                    && buy_token == Some(weth)
                    && amount == Some(1337.into())
                    && kind == Some(OrderKind::Sell)
            })
            .times(1)
            .returning(|_, _, _, _| {
                Ok((
                    0.into(),
                    DateTime::<Utc>::from_utc(NaiveDateTime::from_timestamp(0, 0), Utc),
                ))
            });

        let eth_aware = EthAdapter { calculator, weth };
        assert!(eth_aware
            .min_fee(
                token,
                Some(BUY_ETH_ADDRESS),
                Some(1337.into()),
                Some(OrderKind::Sell)
            )
            .await
            .is_ok());
    }

    #[tokio::test]
    async fn eth_aware_is_valid_fee() {
        let weth = H160([0x42; 20]);
        let token = H160([0x21; 20]);
        let mut calculator = MockMinFeeCalculating::default();
        calculator
            .expect_is_valid_fee()
            .withf(move |&sell_token, &fee| sell_token == token && fee == 42.into())
            .times(1)
            .returning(|_, _| true);

        let eth_aware = EthAdapter { calculator, weth };
        assert!(eth_aware.is_valid_fee(token, 42.into()).await);
    }

    impl MinFeeCalculator {
        fn new_for_test(
            gas_estimator: Arc<dyn GasPriceEstimating>,
            price_estimator: Arc<dyn PriceEstimating>,
            now: Box<dyn Fn() -> DateTime<Utc> + Send + Sync>,
        ) -> Self {
            Self {
                gas_estimator,
                price_estimator,
                native_token: Default::default(),
                measurements: Arc::new(InMemoryFeeStore::default()),
                now,
                fee_factor: 1.0,
                bad_token_detector: Arc::new(ListBasedDetector::deny_list(Vec::new())),
            }
        }
    }

    #[tokio::test]
    async fn accepts_min_fee_if_validated_before_expiry() {
        let gas_price = Arc::new(Mutex::new(100.0));
        let time = Arc::new(Mutex::new(Utc::now()));

        let gas_price_estimator = Arc::new(FakeGasPriceEstimator(gas_price.clone()));
        let price_estimator = Arc::new(FakePriceEstimator(num::one()));
        let time_copy = time.clone();
        let now = move || *time_copy.lock().unwrap();

        let fee_estimator =
            MinFeeCalculator::new_for_test(gas_price_estimator, price_estimator, Box::new(now));

        let token = H160::from_low_u64_be(1);
        let (fee, expiry) = fee_estimator
            .min_fee(token, None, None, None)
            .await
            .unwrap();

        // Gas price increase after measurement
        *gas_price.lock().unwrap() *= 2.0;

        // fee is valid before expiry
        *time.lock().unwrap() = expiry - Duration::seconds(10);
        assert!(fee_estimator.is_valid_fee(token, fee).await);

        // fee is invalid for some uncached token
        let token = H160::from_low_u64_be(2);
        assert!(!fee_estimator.is_valid_fee(token, fee).await);
    }

    #[tokio::test]
    async fn accepts_fee_if_higher_than_current_min_fee() {
        let gas_price = Arc::new(Mutex::new(100.0));

        let gas_price_estimator = Arc::new(FakeGasPriceEstimator(gas_price.clone()));
        let price_estimator = Arc::new(FakePriceEstimator(num::one()));

        let fee_estimator = MinFeeCalculator::new_for_test(
            gas_price_estimator,
            price_estimator,
            Box::new(Utc::now),
        );

        let token = H160::from_low_u64_be(1);
        let (fee, _) = fee_estimator
            .min_fee(token, None, None, None)
            .await
            .unwrap();

        let lower_fee = fee - U256::one();

        // slightly lower fee is not valid
        assert!(!fee_estimator.is_valid_fee(token, lower_fee).await);

        // Gas price reduces, and slightly lower fee is now valid
        *gas_price.lock().unwrap() /= 2.0;
        assert!(fee_estimator.is_valid_fee(token, lower_fee).await);
    }

    #[tokio::test]
    async fn fails_for_unsupported_tokens() {
        let unsupported_token = H160::from_low_u64_be(1);
        let supported_token = H160::from_low_u64_be(2);

        let gas_price_estimator = Arc::new(FakeGasPriceEstimator(Arc::new(Mutex::new(100.0))));
        let price_estimator = Arc::new(FakePriceEstimator(num::one()));

        let fee_estimator = MinFeeCalculator {
            price_estimator,
            gas_estimator: gas_price_estimator,
            native_token: Default::default(),
            measurements: Arc::new(InMemoryFeeStore::default()),
            now: Box::new(Utc::now),
            fee_factor: 1.0,
            bad_token_detector: Arc::new(ListBasedDetector::deny_list(vec![unsupported_token])),
        };

        // Selling unsupported token
        assert!(matches!(
            fee_estimator
                .min_fee(
                    unsupported_token,
                    Some(supported_token),
                    Some(100.into()),
                    Some(OrderKind::Sell)
                )
                .await,
            Err(MinFeeCalculationError::UnsupportedToken(t)) if t == unsupported_token
        ));

        // Buying unsupported token
        assert!(matches!(
            fee_estimator
                .min_fee(
                    supported_token,
                    Some(unsupported_token),
                    Some(100.into()),
                    Some(OrderKind::Sell)
                )
                .await,
            Err(MinFeeCalculationError::UnsupportedToken(t)) if t == unsupported_token
        ));
    }
}
