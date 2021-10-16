use anyhow::Result;
use chrono::{DateTime, Duration, Utc};
use gas_estimation::GasPriceEstimating;
use model::{
    app_id::AppId,
    order::{OrderKind, BUY_ETH_ADDRESS},
};
use primitive_types::{H160, U256};
use shared::{
    bad_token::BadTokenDetecting,
    price_estimation::{self, ensure_token_supported, PriceEstimating, PriceEstimationError},
};
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

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
    partner_additional_fee_factors: HashMap<AppId, f64>,
    native_token_price_estimation_amount: U256,
}

#[cfg_attr(test, mockall::automock)]
#[async_trait::async_trait]
pub trait MinFeeCalculating: Send + Sync {
    /// Returns the minimum amount of fee required to accept an order selling
    /// the specified order and an expiry date for the estimate. The returned
    /// amount applies configured "fee factors" for subsidizing user trades.
    ///
    /// Returns an error if there is some estimation error and `Ok(None)` if no
    /// information about the given token exists
    async fn compute_subsidized_min_fee(
        &self,
        sell_token: H160,
        buy_token: Option<H160>,
        amount: Option<U256>,
        kind: Option<OrderKind>,
        app_data: Option<AppId>,
    ) -> Result<Measurement, PriceEstimationError>;

    /// Validates that the given subsidized fee is enough to process an order for the given token.
    /// Returns current fee estimate (i.e., unsubsidized fee) if the given subsidized fee passes
    /// a check. Returns `Err` if the check failed.
    async fn get_unsubsidized_min_fee(
        &self,
        sell_token: H160,
        fee: U256,
        app_data: Option<AppId>,
    ) -> Result<U256, ()>;
}

#[cfg_attr(test, mockall::automock)]
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
    async fn read_fee_measurement(
        &self,
        sell_token: H160,
        buy_token: Option<H160>,
        amount: Option<U256>,
        kind: Option<OrderKind>,
        min_expiry: DateTime<Utc>,
    ) -> Result<Option<U256>>;
}

const GAS_PER_ORDER: f64 = 300_000.0;

// We use a longer validity internally for persistence to avoid writing a value to storage on every request
// This way we can serve a previous estimate if the same token is queried again shortly after
const STANDARD_VALIDITY_FOR_FEE_IN_SEC: i64 = 60;
const PERSISTED_VALIDITY_FOR_FEE_IN_SEC: i64 = 120;

fn normalize_buy_token(buy_token: H160, weth: H160) -> H160 {
    if buy_token == BUY_ETH_ADDRESS {
        weth
    } else {
        buy_token
    }
}

impl EthAwareMinFeeCalculator {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        price_estimator: Arc<dyn PriceEstimating>,
        gas_estimator: Arc<dyn GasPriceEstimating>,
        native_token: H160,
        measurements: Arc<dyn MinFeeStoring>,
        fee_factor: f64,
        bad_token_detector: Arc<dyn BadTokenDetecting>,
        partner_additional_fee_factors: HashMap<AppId, f64>,
        native_token_price_estimation_amount: U256,
    ) -> Self {
        Self {
            calculator: MinFeeCalculator::new(
                price_estimator,
                gas_estimator,
                native_token,
                measurements,
                fee_factor,
                bad_token_detector,
                partner_additional_fee_factors,
                native_token_price_estimation_amount,
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
    async fn compute_subsidized_min_fee(
        &self,
        sell_token: H160,
        buy_token: Option<H160>,
        amount: Option<U256>,
        kind: Option<OrderKind>,
        app_data: Option<AppId>,
    ) -> Result<Measurement, PriceEstimationError> {
        self.calculator
            .compute_subsidized_min_fee(
                sell_token,
                buy_token.map(|token| normalize_buy_token(token, self.weth)),
                amount,
                kind,
                app_data,
            )
            .await
    }

    async fn get_unsubsidized_min_fee(
        &self,
        sell_token: H160,
        fee: U256,
        app_data: Option<AppId>,
    ) -> Result<U256, ()> {
        self.calculator
            .get_unsubsidized_min_fee(sell_token, fee, app_data)
            .await
    }
}

impl MinFeeCalculator {
    #[allow(clippy::too_many_arguments)]
    fn new(
        price_estimator: Arc<dyn PriceEstimating>,
        gas_estimator: Arc<dyn GasPriceEstimating>,
        native_token: H160,
        measurements: Arc<dyn MinFeeStoring>,
        fee_factor: f64,
        bad_token_detector: Arc<dyn BadTokenDetecting>,
        partner_additional_fee_factors: HashMap<AppId, f64>,
        native_token_price_estimation_amount: U256,
    ) -> Self {
        Self {
            price_estimator,
            gas_estimator,
            native_token,
            measurements,
            now: Box::new(Utc::now),
            fee_factor,
            bad_token_detector,
            partner_additional_fee_factors,
            native_token_price_estimation_amount,
        }
    }

    /// Computes unsubsidized min fee.
    async fn compute_unsubsidized_min_fee(
        &self,
        sell_token: H160,
        buy_token: Option<H160>,
        amount: Option<U256>,
        kind: Option<OrderKind>,
    ) -> Result<U256, PriceEstimationError> {
        let gas_price = self.gas_estimator.estimate().await?;
        tracing::debug!(
            "estimated effective gas price of {:.2} Gwei",
            gas_price / 1e9
        );

        let gas_amount =
            if let (Some(buy_token), Some(amount), Some(kind)) = (buy_token, amount, kind) {
                self.price_estimator
                    .estimate(&price_estimation::Query {
                        sell_token,
                        buy_token,
                        in_amount: amount,
                        kind,
                    })
                    .await?
                    .gas
                    .to_f64_lossy()
            } else {
                GAS_PER_ORDER
            };

        let fee_in_eth = gas_price * gas_amount;
        let query = price_estimation::Query {
            sell_token,
            buy_token: self.native_token,
            in_amount: self.native_token_price_estimation_amount,
            kind: OrderKind::Buy,
        };
        let estimate = self.price_estimator.estimate(&query).await?;
        let price = estimate.price_in_sell_token_f64(&query);

        tracing::debug!(
            "computed unsubsidized fee amount of {} ETH at a price of {} ETH/{:?}",
            fee_in_eth,
            price,
            sell_token,
        );

        Ok(U256::from_f64_lossy(fee_in_eth * price))
    }

    fn apply_fee_factor(&self, fee: U256, app_data: Option<AppId>) -> U256 {
        let factor = app_data
            .and_then(|app_data| self.partner_additional_fee_factors.get(&app_data))
            .copied()
            .unwrap_or(1.0)
            * self.fee_factor;
        U256::from_f64_lossy(fee.to_f64_lossy() * factor)
    }
}

#[async_trait::async_trait]
impl MinFeeCalculating for MinFeeCalculator {
    async fn compute_subsidized_min_fee(
        &self,
        sell_token: H160,
        buy_token: Option<H160>,
        amount: Option<U256>,
        kind: Option<OrderKind>,
        app_data: Option<AppId>,
    ) -> Result<Measurement, PriceEstimationError> {
        ensure_token_supported(sell_token, self.bad_token_detector.as_ref()).await?;
        if let Some(buy_token) = buy_token {
            ensure_token_supported(buy_token, self.bad_token_detector.as_ref()).await?;
        }

        let now = (self.now)();
        let official_valid_until = now + Duration::seconds(STANDARD_VALIDITY_FOR_FEE_IN_SEC);
        let internal_valid_until = now + Duration::seconds(PERSISTED_VALIDITY_FOR_FEE_IN_SEC);

        tracing::debug!(
            "computing subsidized fee for {:?}",
            (sell_token, buy_token, amount, kind, app_data),
        );

        let unsubsidized_min_fee = if let Ok(Some(past_fee)) = self
            .measurements
            .read_fee_measurement(sell_token, buy_token, amount, kind, official_valid_until)
            .await
        {
            tracing::debug!("using existing fee measurement {}", past_fee);
            past_fee
        } else {
            let current_fee = self
                .compute_unsubsidized_min_fee(sell_token, buy_token, amount, kind)
                .await?;

            if let Err(err) = self
                .measurements
                .save_fee_measurement(
                    sell_token,
                    buy_token,
                    amount,
                    kind,
                    internal_valid_until,
                    current_fee,
                )
                .await
            {
                tracing::warn!(?err, "error saving fee measurement");
            }

            tracing::debug!("using new fee measurement {}", current_fee);
            current_fee
        };

        let subsidized_min_fee = self.apply_fee_factor(unsubsidized_min_fee, app_data);
        tracing::debug!(
            "computed subsidized fee of {:?}",
            (subsidized_min_fee, sell_token),
        );

        Ok((subsidized_min_fee, official_valid_until))
    }

    async fn get_unsubsidized_min_fee(
        &self,
        sell_token: H160,
        fee: U256,
        app_data: Option<AppId>,
    ) -> Result<U256, ()> {
        if let Ok(Some(past_fee)) = self
            .measurements
            .read_fee_measurement(sell_token, None, None, None, (self.now)())
            .await
        {
            if fee >= self.apply_fee_factor(past_fee, app_data) {
                return Ok(std::cmp::max(fee, past_fee));
            }
        }

        if let Ok(current_fee) = self
            .compute_unsubsidized_min_fee(sell_token, None, None, None)
            .await
        {
            if fee >= self.apply_fee_factor(current_fee, app_data) {
                return Ok(std::cmp::max(fee, current_fee));
            }
        }

        Err(())
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

    async fn read_fee_measurement(
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
    use maplit::hashmap;
    use mockall::{predicate::*, Sequence};
    use shared::{
        bad_token::list_based::ListBasedDetector, gas_price_estimation::FakeGasPriceEstimator,
        price_estimation::mocks::FakePriceEstimator,
    };
    use std::sync::Arc;

    use super::*;

    #[tokio::test]
    async fn eth_aware_min_fees() {
        let weth = H160([0x42; 20]);
        let token = H160([0x21; 20]);
        let mut calculator = MockMinFeeCalculating::default();
        calculator
            .expect_compute_subsidized_min_fee()
            .withf(move |&sell_token, &buy_token, &amount, &kind, &app_data| {
                sell_token == token
                    && buy_token == Some(weth)
                    && amount == Some(1337.into())
                    && kind == Some(OrderKind::Sell)
                    && app_data == None
            })
            .times(1)
            .returning(|_, _, _, _, _| {
                Ok((
                    0.into(),
                    DateTime::<Utc>::from_utc(NaiveDateTime::from_timestamp(0, 0), Utc),
                ))
            });

        let eth_aware = EthAdapter { calculator, weth };
        assert!(eth_aware
            .compute_subsidized_min_fee(
                token,
                Some(BUY_ETH_ADDRESS),
                Some(1337.into()),
                Some(OrderKind::Sell),
                None,
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
            .expect_get_unsubsidized_min_fee()
            .withf(move |&sell_token, &fee, &app_data| {
                sell_token == token && fee == 42.into() && app_data == Default::default()
            })
            .times(1)
            .returning(|_, fee, _| Ok(fee));

        let eth_aware = EthAdapter { calculator, weth };
        assert_eq!(
            eth_aware
                .get_unsubsidized_min_fee(token, 42.into(), Default::default())
                .await,
            Ok(42.into())
        );
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
                partner_additional_fee_factors: hashmap! {},
                native_token_price_estimation_amount: 1.into(),
            }
        }
    }

    #[tokio::test]
    async fn accepts_min_fee_if_validated_before_expiry() {
        let gas_price = Arc::new(Mutex::new(100.0));
        let time = Arc::new(Mutex::new(Utc::now()));

        let gas_price_estimator = Arc::new(FakeGasPriceEstimator(gas_price.clone()));
        let price_estimator = FakePriceEstimator(price_estimation::Estimate {
            out_amount: 1.into(),
            gas: 1.into(),
        });
        let time_copy = time.clone();
        let now = move || *time_copy.lock().unwrap();

        let fee_estimator = MinFeeCalculator::new_for_test(
            gas_price_estimator,
            Arc::new(price_estimator),
            Box::new(now),
        );

        let token = H160::from_low_u64_be(1);
        let (fee, expiry) = fee_estimator
            .compute_subsidized_min_fee(token, None, None, None, None)
            .await
            .unwrap();
        // Gas price increase after measurement
        *gas_price.lock().unwrap() *= 2.0;

        // fee is valid before expiry
        *time.lock().unwrap() = expiry - Duration::seconds(10);
        assert!(fee_estimator
            .get_unsubsidized_min_fee(token, fee, None)
            .await
            .is_ok());

        // fee is invalid for some uncached token
        let token = H160::from_low_u64_be(2);
        assert!(!fee_estimator
            .get_unsubsidized_min_fee(token, fee, None)
            .await
            .is_ok());
    }

    #[tokio::test]
    async fn accepts_fee_if_higher_than_current_min_fee() {
        let gas_price = Arc::new(Mutex::new(100.0));

        let gas_price_estimator = Arc::new(FakeGasPriceEstimator(gas_price.clone()));
        let price_estimator = FakePriceEstimator(price_estimation::Estimate {
            out_amount: 1.into(),
            gas: 1.into(),
        });

        let fee_estimator = MinFeeCalculator::new_for_test(
            gas_price_estimator,
            Arc::new(price_estimator),
            Box::new(Utc::now),
        );

        let token = H160::from_low_u64_be(1);
        let (fee, _) = fee_estimator
            .compute_subsidized_min_fee(token, None, None, None, None)
            .await
            .unwrap();

        dbg!(fee);
        let lower_fee = fee - U256::one();
        // slightly lower fee is not valid
        assert!(fee_estimator
            .get_unsubsidized_min_fee(token, lower_fee, None)
            .await
            .is_err());

        // Gas price reduces, and slightly lower fee is now valid
        *gas_price.lock().unwrap() /= 2.0;
        assert!(fee_estimator
            .get_unsubsidized_min_fee(token, lower_fee, None)
            .await
            .is_ok());
    }

    #[tokio::test]
    async fn fails_for_unsupported_tokens() {
        let unsupported_token = H160::from_low_u64_be(1);
        let supported_token = H160::from_low_u64_be(2);

        let gas_price_estimator = Arc::new(FakeGasPriceEstimator(Arc::new(Mutex::new(100.0))));
        let price_estimator = Arc::new(FakePriceEstimator(price_estimation::Estimate {
            out_amount: 1.into(),
            gas: 1000.into(),
        }));

        let fee_estimator = MinFeeCalculator {
            price_estimator,
            gas_estimator: gas_price_estimator,
            native_token: Default::default(),
            measurements: Arc::new(InMemoryFeeStore::default()),
            now: Box::new(Utc::now),
            fee_factor: 1.0,
            bad_token_detector: Arc::new(ListBasedDetector::deny_list(vec![unsupported_token])),
            partner_additional_fee_factors: hashmap! {},
            native_token_price_estimation_amount: 1.into(),
        };

        // Selling unsupported token
        assert!(matches!(
            fee_estimator
                .compute_subsidized_min_fee(
                    unsupported_token,
                    Some(supported_token),
                    Some(100.into()),
                    Some(OrderKind::Sell),
                    None,
                )
                .await,
            Err(PriceEstimationError::UnsupportedToken(t)) if t == unsupported_token
        ));

        // Buying unsupported token
        assert!(matches!(
            fee_estimator
                .compute_subsidized_min_fee(
                    supported_token,
                    Some(unsupported_token),
                    Some(100.into()),
                    Some(OrderKind::Sell),
                    None,
                )
                .await,
            Err(PriceEstimationError::UnsupportedToken(t)) if t == unsupported_token
        ));
    }

    #[tokio::test]
    async fn is_valid_fee() {
        let sell_token = H160::from_low_u64_be(1);

        let gas_price_estimator = Arc::new(FakeGasPriceEstimator(Arc::new(Mutex::new(100.0))));
        let price_estimator = Arc::new(FakePriceEstimator(price_estimation::Estimate {
            out_amount: 1.into(),
            gas: 1000.into(),
        }));
        let app_data = AppId([1u8; 32]);
        let fee_estimator = MinFeeCalculator {
            price_estimator,
            gas_estimator: gas_price_estimator,
            native_token: Default::default(),
            measurements: Arc::new(InMemoryFeeStore::default()),
            now: Box::new(Utc::now),
            fee_factor: 1.0,
            bad_token_detector: Arc::new(ListBasedDetector::deny_list(vec![])),
            partner_additional_fee_factors: hashmap! { app_data => 0.5 },
            native_token_price_estimation_amount: 1.into(),
        };
        let (fee, _) = fee_estimator
            .compute_subsidized_min_fee(sell_token, None, None, None, Some(app_data))
            .await
            .unwrap();
        assert_eq!(
            fee_estimator
                .get_unsubsidized_min_fee(sell_token, fee, Some(app_data))
                .await,
            Ok(fee * 2)
        );
        assert!(fee_estimator
            .get_unsubsidized_min_fee(sell_token, fee, None)
            .await
            .is_err());
        let lower_fee = fee - U256::one();
        assert!(fee_estimator
            .get_unsubsidized_min_fee(sell_token, lower_fee, Some(app_data))
            .await
            .is_err());
    }

    #[tokio::test]
    async fn applies_fee_factor_to_past_and_new_fees() {
        let sell_token = H160::from_low_u64_be(1);
        let native_token_price_estimation_amount = 100.;
        let sell_token_price = 1.25;
        let gas_estimate = 42.;

        let unsubsidized_min_fee =
            U256::from_f64_lossy(GAS_PER_ORDER * sell_token_price * gas_estimate);

        let gas_estimator = Arc::new(FakeGasPriceEstimator(Arc::new(Mutex::new(42.))));
        let price_estimator = Arc::new(FakePriceEstimator(price_estimation::Estimate {
            out_amount: U256::from_f64_lossy(
                native_token_price_estimation_amount * sell_token_price,
            ),
            gas: 1337.into(),
        }));

        let mut measurements = MockMinFeeStoring::new();
        let mut seq = Sequence::new();
        measurements
            .expect_read_fee_measurement()
            .times(1)
            .in_sequence(&mut seq)
            .with(eq(sell_token), eq(None), eq(None), eq(None), always())
            .returning(|_, _, _, _, _| Ok(None));
        measurements
            .expect_save_fee_measurement()
            .times(1)
            .in_sequence(&mut seq)
            .with(
                eq(sell_token),
                eq(None),
                eq(None),
                eq(None),
                always(),
                eq(unsubsidized_min_fee),
            )
            .returning(|_, _, _, _, _, _| Ok(()));
        measurements
            .expect_read_fee_measurement()
            .times(1)
            .in_sequence(&mut seq)
            .with(eq(sell_token), eq(None), eq(None), eq(None), always())
            .returning(move |_, _, _, _, _| Ok(Some(unsubsidized_min_fee)));

        let app_data = AppId([1u8; 32]);
        let fee_estimator = MinFeeCalculator {
            price_estimator,
            gas_estimator,
            native_token: Default::default(),
            measurements: Arc::new(measurements),
            now: Box::new(Utc::now),
            fee_factor: 0.8,
            bad_token_detector: Arc::new(ListBasedDetector::deny_list(vec![])),
            partner_additional_fee_factors: hashmap! { app_data => 0.5 },
            native_token_price_estimation_amount: U256::from_f64_lossy(
                native_token_price_estimation_amount,
            ),
        };

        let (fee, _) = fee_estimator
            .compute_subsidized_min_fee(sell_token, None, None, None, Some(app_data))
            .await
            .unwrap();
        assert_eq!(
            fee,
            U256::from_f64_lossy(unsubsidized_min_fee.to_f64_lossy() * 0.8 * 0.5)
        );

        let (fee, _) = fee_estimator
            .compute_subsidized_min_fee(sell_token, None, None, None, None)
            .await
            .unwrap();
        assert_eq!(
            fee,
            U256::from_f64_lossy(unsubsidized_min_fee.to_f64_lossy() * 0.8)
        );
    }
}
