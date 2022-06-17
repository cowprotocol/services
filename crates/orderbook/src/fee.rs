use anyhow::Result;
use chrono::{DateTime, Duration, NaiveDateTime, Utc, MAX_DATETIME};
use futures::future::TryFutureExt;
use gas_estimation::GasPriceEstimating;
use model::{
    app_id::AppId,
    order::OrderKind,
    quote::{OrderQuoteSide, SellAmount},
};
use primitive_types::{H160, U256};
use shared::{
    bad_token::BadTokenDetecting,
    price_estimation::{
        self, ensure_token_supported, native::native_single_estimate, PriceEstimating,
        PriceEstimationError,
    },
    price_estimation::{native::NativePriceEstimating, single_estimate},
};
use std::{
    collections::HashSet,
    sync::{Arc, Mutex},
};

use crate::{
    fee_subsidy::{FeeParameters, FeeSubsidizing},
    order_quoting::QuoteParameters,
};

pub type Measurement = (U256, DateTime<Utc>);

pub struct MinFeeCalculator {
    price_estimator: Arc<dyn PriceEstimating>,
    gas_estimator: Arc<dyn GasPriceEstimating>,
    measurements: Arc<dyn MinFeeStoring>,
    now: Box<dyn Fn() -> DateTime<Utc> + Send + Sync>,
    bad_token_detector: Arc<dyn BadTokenDetecting>,
    fee_subsidy: Arc<dyn FeeSubsidizing>,
    native_price_estimator: Arc<dyn NativePriceEstimating>,
    liquidity_order_owners: HashSet<H160>,
    store_computed_fees: bool,
}

#[derive(Debug, Default, Clone, Copy, Eq, PartialEq)]
pub struct FeeData {
    pub sell_token: H160,
    pub buy_token: H160,
    // For sell orders this is the sell amount before fees.
    pub amount: U256,
    pub kind: OrderKind,
}

#[derive(Debug)]
pub enum GetUnsubsidizedMinFeeError {
    InsufficientFee,
    PriceEstimationError(PriceEstimationError),
    Other(anyhow::Error),
}

impl From<anyhow::Error> for GetUnsubsidizedMinFeeError {
    fn from(err: anyhow::Error) -> Self {
        Self::Other(err)
    }
}

// The user address is mandatory. If some code does not know the user it can pass the 0 address
// which is guaranteed to not have any balance for the cow token.
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
        fee_data: FeeData,
        app_data: AppId,
        user: H160,
    ) -> Result<Measurement, PriceEstimationError>;

    /// Validates that the given subsidized fee is enough to process an order for the given token.
    /// Returns current fee estimate (i.e., unsubsidized fee) if the given subsidized fee passes
    /// a check. Returns `Err` if the check failed.
    async fn get_unsubsidized_min_fee(
        &self,
        fee_data: FeeData,
        app_data: AppId,
        subsidized_fee: U256,
        user: H160,
    ) -> Result<FeeParameters, GetUnsubsidizedMinFeeError>;
}

#[cfg_attr(test, mockall::automock)]
#[async_trait::async_trait]
pub trait MinFeeStoring: Send + Sync {
    /// Stores the given measurement. Returns an error if this fails
    async fn save_fee_measurement(
        &self,
        fee_data: FeeData,
        expiry: DateTime<Utc>,
        estimate: FeeParameters,
    ) -> Result<()>;

    /// Returns lowest previously stored measurements that hasn't expired. FeeData has to match
    /// exactly.
    async fn find_measurement_exact(
        &self,
        fee_data: FeeData,
        min_expiry: DateTime<Utc>,
    ) -> Result<Option<FeeParameters>>;

    /// Returns lowest previously stored measurements that hasn't expired. FeeData has to match
    /// exactly except for the amount which is allowed to be larger than the amount in fee data.
    async fn find_measurement_including_larger_amount(
        &self,
        fee_data: FeeData,
        min_expiry: DateTime<Utc>,
    ) -> Result<Option<FeeParameters>>;
}

// We use a longer validity internally for persistence to avoid writing a value to storage on every request
// This way we can serve a previous estimate if the same token is queried again shortly after
const STANDARD_VALIDITY_FOR_FEE_IN_SEC: i64 = 60;
const PERSISTED_VALIDITY_FOR_FEE_IN_SEC: i64 = 120;

impl MinFeeCalculator {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        price_estimator: Arc<dyn PriceEstimating>,
        gas_estimator: Arc<dyn GasPriceEstimating>,
        measurements: Arc<dyn MinFeeStoring>,
        bad_token_detector: Arc<dyn BadTokenDetecting>,
        fee_subsidy: Arc<dyn FeeSubsidizing>,
        native_price_estimator: Arc<dyn NativePriceEstimating>,
        liquidity_order_owners: HashSet<H160>,
        store_computed_fees: bool,
    ) -> Self {
        Self {
            price_estimator,
            gas_estimator,
            measurements,
            now: Box::new(Utc::now),
            bad_token_detector,
            fee_subsidy,
            native_price_estimator,
            liquidity_order_owners,
            store_computed_fees,
        }
    }

    /// Computes unsubsidized min fee.
    async fn compute_unsubsidized_min_fee(
        &self,
        fee_data: FeeData,
    ) -> Result<FeeParameters, PriceEstimationError> {
        let buy_token_query = price_estimation::Query {
            sell_token: fee_data.sell_token,
            buy_token: fee_data.buy_token,
            in_amount: fee_data.amount,
            kind: fee_data.kind,
        };
        let (gas_estimate, buy_token_estimate, sell_token_price) = futures::try_join!(
            self.gas_estimator
                .estimate()
                .map_err(PriceEstimationError::from),
            single_estimate(self.price_estimator.as_ref(), &buy_token_query),
            native_single_estimate(self.native_price_estimator.as_ref(), &fee_data.sell_token),
        )?;
        let gas_price = gas_estimate.effective_gas_price();
        let gas_amount = buy_token_estimate.gas as f64;
        let fee_parameters = FeeParameters {
            gas_amount,
            gas_price,
            sell_token_price,
        };

        let fee_in_eth = gas_price * gas_amount;
        let fee_in_sell_token = fee_parameters.unsubsidized();
        tracing::debug!(
            ?fee_data, %gas_price, %gas_amount, %sell_token_price,
            %fee_in_eth, %fee_in_sell_token,
            "unsubsidized fee amount"
        );

        Ok(fee_parameters)
    }
}

#[async_trait::async_trait]
impl MinFeeCalculating for MinFeeCalculator {
    async fn compute_subsidized_min_fee(
        &self,
        fee_data: FeeData,
        app_data: AppId,
        user: H160,
    ) -> Result<Measurement, PriceEstimationError> {
        if fee_data.buy_token == fee_data.sell_token {
            return Ok((U256::zero(), MAX_DATETIME));
        }
        if self.liquidity_order_owners.contains(&user) {
            return Ok((U256::zero(), MAX_DATETIME));
        }

        ensure_token_supported(fee_data.sell_token, self.bad_token_detector.as_ref()).await?;
        ensure_token_supported(fee_data.buy_token, self.bad_token_detector.as_ref()).await?;

        let now = (self.now)();
        let mut official_valid_until = now + Duration::seconds(STANDARD_VALIDITY_FOR_FEE_IN_SEC);
        let internal_valid_until = now + Duration::seconds(PERSISTED_VALIDITY_FOR_FEE_IN_SEC);

        tracing::debug!(?fee_data, ?app_data, ?user, "computing subsidized fee",);

        let subsidy = async {
            self.fee_subsidy
                .subsidy(quote_parameters(&fee_data, user, app_data))
                .await
                .map_err(PriceEstimationError::Other)
        };
        let unsubsidized_min_fee = async {
            if let Some(past_fee) = self
                .measurements
                .find_measurement_exact(fee_data, official_valid_until)
                .await?
            {
                tracing::debug!("using existing fee measurement {:?}", past_fee);
                Ok(past_fee)
            } else {
                let current_fee = self.compute_unsubsidized_min_fee(fee_data).await?;

                if self.store_computed_fees {
                    if let Err(err) = self
                        .measurements
                        .save_fee_measurement(fee_data, internal_valid_until, current_fee)
                        .await
                    {
                        tracing::warn!(?err, "error saving fee measurement");
                    }
                } else {
                    tracing::debug!("skip saving fee measurement");
                }

                tracing::debug!("using new fee measurement {:?}", current_fee);
                Ok(current_fee)
            }
        };

        let (subsidy, unsubsidized_min_fee) =
            futures::future::try_join(subsidy, unsubsidized_min_fee).await?;

        let subsidized_min_fee = unsubsidized_min_fee.subsidized(&subsidy);
        tracing::debug!(
            "computed subsidized fee of {:?}",
            (subsidized_min_fee, fee_data.sell_token),
        );

        if !self.store_computed_fees {
            // Set an expired timeout to signal that this fee estimate is only indicative
            // and not supposed to be used to create actual orders with it.
            //
            // Technically we could only set this sentinel when we had to compute a fee
            // estimate from scratch but to make it more consistent for the user we'll always
            // return this value when the fee estimate will not end up in the database.
            official_valid_until = DateTime::from_utc(NaiveDateTime::from_timestamp(0, 0), Utc);
        }

        Ok((subsidized_min_fee, official_valid_until))
    }

    async fn get_unsubsidized_min_fee(
        &self,
        fee_data: FeeData,
        app_data: AppId,
        subsidized_fee: U256,
        user: H160,
    ) -> Result<FeeParameters, GetUnsubsidizedMinFeeError> {
        if self.liquidity_order_owners.contains(&user) {
            return Ok(FeeParameters::default());
        }

        let subsidy = self
            .fee_subsidy
            .subsidy(quote_parameters(&fee_data, user, app_data));
        let past_fee = self
            .measurements
            .find_measurement_including_larger_amount(fee_data, (self.now)());
        let (subsidy, past_fee) = futures::future::try_join(subsidy, past_fee).await?;
        // When validating we allow fees taken for larger amounts because as the amount increases
        // the fee increases too because it is worth to trade off more gas use for a slightly better
        // price. Thus it is acceptable if the new order has an amount <= an existing fee
        // measurement.
        // We do not use "exact" here because for sell orders the final sell_amount might have been
        // calculated as original_sell_amount + fee_response or sell_amount
        // (sell amount before vs after fee).
        // Once we have removed the `fee` route and moved all fee requests to the `quote` route we
        // might no longer need this workaround as we will know exactly how the amounts in the order
        // have been picked.
        if let Some(past_fee) = past_fee {
            tracing::debug!("found past fee {:?}", past_fee);
            if subsidized_fee >= past_fee.subsidized(&subsidy) {
                tracing::debug!("given fee matches past fee");
                return Ok(past_fee);
            } else {
                tracing::debug!("given fee does not match past fee");
            }
        }

        let current_fee = self
            .compute_unsubsidized_min_fee(fee_data)
            .await
            .map_err(GetUnsubsidizedMinFeeError::PriceEstimationError)?;
        tracing::debug!("estimated new fee {:?}", current_fee);
        if subsidized_fee >= current_fee.subsidized(&subsidy) {
            tracing::debug!("given fee matches new fee");
            Ok(current_fee)
        } else {
            tracing::debug!("given fee does not match new fee");
            Err(GetUnsubsidizedMinFeeError::InsufficientFee)
        }
    }
}

struct FeeMeasurement {
    fee_data: FeeData,
    expiry: DateTime<Utc>,
    estimate: FeeParameters,
}

#[derive(Default)]
struct InMemoryFeeStore(Mutex<Vec<FeeMeasurement>>);

#[async_trait::async_trait]
impl MinFeeStoring for InMemoryFeeStore {
    async fn save_fee_measurement(
        &self,
        fee_data: FeeData,
        expiry: DateTime<Utc>,
        estimate: FeeParameters,
    ) -> Result<()> {
        self.0
            .lock()
            .expect("Thread holding Mutex panicked")
            .push(FeeMeasurement {
                fee_data,
                expiry,
                estimate,
            });
        Ok(())
    }

    async fn find_measurement_exact(
        &self,
        fee_data: FeeData,
        min_expiry: DateTime<Utc>,
    ) -> Result<Option<FeeParameters>> {
        let guard = self.0.lock().expect("Thread holding Mutex panicked");
        Ok(guard
            .iter()
            .filter(|measurement| {
                measurement.expiry >= min_expiry && measurement.fee_data == fee_data
            })
            .map(|measurement| measurement.estimate)
            .min_by_key(|estimate| estimate.unsubsidized()))
    }

    async fn find_measurement_including_larger_amount(
        &self,
        fee_data: FeeData,
        min_expiry: DateTime<Utc>,
    ) -> Result<Option<FeeParameters>> {
        let guard = self.0.lock().expect("Thread holding Mutex panicked");
        Ok(guard
            .iter()
            .filter(|measurement| {
                measurement.expiry >= min_expiry
                    && measurement.fee_data.sell_token == fee_data.sell_token
                    && measurement.fee_data.buy_token == fee_data.buy_token
                    && measurement.fee_data.kind == fee_data.kind
                    && measurement.fee_data.amount >= fee_data.amount
            })
            .map(|measurement| measurement.estimate)
            .min_by_key(|estimate| estimate.unsubsidized()))
    }
}

fn quote_parameters(fee_data: &FeeData, from: H160, app_data: AppId) -> QuoteParameters {
    QuoteParameters {
        sell_token: fee_data.sell_token,
        buy_token: fee_data.buy_token,
        side: match fee_data.kind {
            OrderKind::Buy => OrderQuoteSide::Buy {
                buy_amount_after_fee: fee_data.amount,
            },
            OrderKind::Sell => OrderQuoteSide::Sell {
                sell_amount: SellAmount::AfterFee {
                    value: fee_data.amount,
                },
            },
        },
        from,
        app_data,
    }
}

#[cfg(test)]
#[allow(clippy::float_cmp)]
mod tests {
    use super::*;
    use crate::fee_subsidy::Subsidy;
    use chrono::Duration;
    use futures::FutureExt;
    use gas_estimation::GasPrice1559;
    use maplit::hashset;
    use mockall::{predicate::*, Sequence};
    use shared::{
        bad_token::list_based::ListBasedDetector, gas_price_estimation::FakeGasPriceEstimator,
        price_estimation::mocks::FakePriceEstimator,
        price_estimation::native::NativePriceEstimator,
    };
    use std::sync::Arc;

    fn create_default_native_token_estimator(
        price_estimator: Arc<dyn PriceEstimating>,
    ) -> Arc<dyn NativePriceEstimating> {
        Arc::new(NativePriceEstimator::new(
            price_estimator,
            Default::default(),
            1.into(),
        ))
    }

    impl MinFeeCalculator {
        fn new_for_test(
            gas_estimator: Arc<dyn GasPriceEstimating>,
            price_estimator: Arc<dyn PriceEstimating>,
            now: Box<dyn Fn() -> DateTime<Utc> + Send + Sync>,
        ) -> Self {
            Self {
                gas_estimator,
                price_estimator: price_estimator.clone(),
                measurements: Arc::new(InMemoryFeeStore::default()),
                now,
                bad_token_detector: Arc::new(ListBasedDetector::deny_list(Vec::new())),
                fee_subsidy: Arc::new(Subsidy::default()),
                native_price_estimator: create_default_native_token_estimator(price_estimator),
                liquidity_order_owners: Default::default(),
                store_computed_fees: true,
            }
        }
    }

    #[tokio::test]
    async fn accepts_min_fee_if_validated_before_expiry() {
        let gas_price = Arc::new(Mutex::new(GasPrice1559 {
            max_fee_per_gas: 100.0,
            max_priority_fee_per_gas: 50.0,
            base_fee_per_gas: 30.0,
        }));
        let time = Arc::new(Mutex::new(Utc::now()));

        let gas_price_estimator = Arc::new(FakeGasPriceEstimator(gas_price.clone()));
        let price_estimator = FakePriceEstimator(price_estimation::Estimate {
            out_amount: 1.into(),
            gas: 1,
        });

        let time_copy = time.clone();
        let now = move || *time_copy.lock().unwrap();

        let fee_estimator = MinFeeCalculator::new_for_test(
            gas_price_estimator,
            Arc::new(price_estimator),
            Box::new(now),
        );

        let token = H160::from_low_u64_be(1);
        let fee_data = FeeData {
            sell_token: token,
            ..Default::default()
        };
        let (fee, expiry) = fee_estimator
            .compute_subsidized_min_fee(fee_data, Default::default(), Default::default())
            .await
            .unwrap();
        // Gas price increase after measurement
        let new_gas_price = gas_price.lock().unwrap().bump(2.0);
        *gas_price.lock().unwrap() = new_gas_price;

        // fee is valid before expiry
        *time.lock().unwrap() = expiry - Duration::seconds(10);
        assert!(fee_estimator
            .get_unsubsidized_min_fee(fee_data, Default::default(), fee, Default::default())
            .await
            .is_ok());

        // fee is invalid for some uncached token
        let token = H160::from_low_u64_be(2);
        assert!(fee_estimator
            .get_unsubsidized_min_fee(
                FeeData {
                    sell_token: token,
                    ..Default::default()
                },
                Default::default(),
                fee,
                Default::default()
            )
            .await
            .is_err());
    }

    #[tokio::test]
    async fn accepts_fee_if_higher_than_current_min_fee() {
        let gas_price = Arc::new(Mutex::new(GasPrice1559 {
            max_fee_per_gas: 100.0,
            max_priority_fee_per_gas: 50.0,
            base_fee_per_gas: 30.0,
        }));

        let gas_price_estimator = Arc::new(FakeGasPriceEstimator(gas_price.clone()));
        let price_estimator = Arc::new(FakePriceEstimator(price_estimation::Estimate {
            out_amount: 1.into(),
            gas: 1,
        }));

        let fee_estimator = MinFeeCalculator::new_for_test(
            gas_price_estimator,
            price_estimator,
            Box::new(Utc::now),
        );

        let token = H160::from_low_u64_be(1);
        let fee_data = FeeData {
            sell_token: token,
            ..Default::default()
        };
        let (fee, _) = fee_estimator
            .compute_subsidized_min_fee(fee_data, Default::default(), Default::default())
            .await
            .unwrap();

        dbg!(fee);
        let lower_fee = fee - 1;
        // slightly lower fee is not valid
        assert!(fee_estimator
            .get_unsubsidized_min_fee(fee_data, Default::default(), lower_fee, Default::default())
            .await
            .is_err());

        // Gas price reduces, and slightly lower fee is now valid
        let new_gas_price = gas_price.lock().unwrap().bump(0.5);
        *gas_price.lock().unwrap() = new_gas_price;
        assert!(fee_estimator
            .get_unsubsidized_min_fee(fee_data, Default::default(), lower_fee, Default::default())
            .await
            .is_ok());
    }

    #[tokio::test]
    async fn fails_for_unsupported_tokens() {
        let unsupported_token = H160::from_low_u64_be(1);
        let supported_token = H160::from_low_u64_be(2);

        let gas_price_estimator =
            Arc::new(FakeGasPriceEstimator(Arc::new(Mutex::new(GasPrice1559 {
                max_fee_per_gas: 100.0,
                max_priority_fee_per_gas: 50.0,
                base_fee_per_gas: 30.0,
            }))));
        let price_estimator = Arc::new(FakePriceEstimator(price_estimation::Estimate {
            out_amount: 1.into(),
            gas: 1000,
        }));
        let native_price_estimator = create_default_native_token_estimator(price_estimator.clone());

        let fee_estimator = MinFeeCalculator {
            price_estimator,
            gas_estimator: gas_price_estimator,
            measurements: Arc::new(InMemoryFeeStore::default()),
            now: Box::new(Utc::now),
            bad_token_detector: Arc::new(ListBasedDetector::deny_list(vec![unsupported_token])),
            fee_subsidy: Arc::new(Subsidy::default()),
            native_price_estimator,
            liquidity_order_owners: Default::default(),
            store_computed_fees: true,
        };

        // Selling unsupported token
        let result = fee_estimator
            .compute_subsidized_min_fee(
                FeeData {
                    sell_token: unsupported_token,
                    buy_token: supported_token,
                    amount: 100.into(),
                    kind: OrderKind::Sell,
                },
                Default::default(),
                Default::default(),
            )
            .await;
        assert!(matches!(
            result,
            Err(PriceEstimationError::UnsupportedToken(t)) if t == unsupported_token
        ));

        // Buying unsupported token
        let result = fee_estimator
            .compute_subsidized_min_fee(
                FeeData {
                    sell_token: supported_token,
                    buy_token: unsupported_token,
                    amount: 100.into(),
                    kind: OrderKind::Sell,
                },
                Default::default(),
                Default::default(),
            )
            .await;
        assert!(matches!(
            result,
            Err(PriceEstimationError::UnsupportedToken(t)) if t == unsupported_token
        ));
    }

    #[tokio::test]
    async fn is_valid_fee() {
        shared::tracing::initialize_for_tests("debug");
        let sell_token = H160::from_low_u64_be(1);
        let fee_data = FeeData {
            sell_token,
            ..Default::default()
        };

        let gas_price_estimator =
            Arc::new(FakeGasPriceEstimator(Arc::new(Mutex::new(GasPrice1559 {
                max_fee_per_gas: 100.0,
                max_priority_fee_per_gas: 50.0,
                base_fee_per_gas: 30.0,
            }))));
        let price_estimator = Arc::new(FakePriceEstimator(price_estimation::Estimate {
            out_amount: 1.into(),
            gas: 1000,
        }));
        let native_price_estimator = create_default_native_token_estimator(price_estimator.clone());
        let app_data = AppId([1u8; 32]);
        let user = H160::zero();
        let fee_estimator = MinFeeCalculator {
            price_estimator,
            gas_estimator: gas_price_estimator,
            measurements: Arc::new(InMemoryFeeStore::default()),
            now: Box::new(Utc::now),
            bad_token_detector: Arc::new(ListBasedDetector::deny_list(vec![])),
            fee_subsidy: Arc::new(Subsidy {
                factor: 0.5,
                ..Default::default()
            }),
            native_price_estimator,
            liquidity_order_owners: Default::default(),
            store_computed_fees: true,
        };

        let (fee, _) = fee_estimator
            .compute_subsidized_min_fee(fee_data, app_data, user)
            .await
            .unwrap();
        assert_eq!(
            fee_estimator
                .get_unsubsidized_min_fee(fee_data, app_data, fee, user)
                .await
                .unwrap()
                .unsubsidized(),
            fee * 2,
        );

        let lower_fee = fee - 1;
        assert!(fee_estimator
            .get_unsubsidized_min_fee(fee_data, app_data, lower_fee, user)
            .await
            .is_err());
    }

    #[tokio::test]
    async fn storing_fees_can_be_disabled() {
        let gas_estimator = Arc::new(FakeGasPriceEstimator(Arc::new(Mutex::new(GasPrice1559 {
            max_fee_per_gas: 100.0,
            max_priority_fee_per_gas: 50.0,
            base_fee_per_gas: 30.0,
        }))));
        let price_estimator = Arc::new(FakePriceEstimator(price_estimation::Estimate {
            out_amount: 1.into(),
            gas: 1000,
        }));
        let native_price_estimator = create_default_native_token_estimator(price_estimator.clone());
        let db = Arc::new(InMemoryFeeStore::default());
        let fee_estimator = MinFeeCalculator {
            price_estimator,
            gas_estimator,
            measurements: db.clone(),
            now: Box::new(Utc::now),
            bad_token_detector: Arc::new(ListBasedDetector::deny_list(vec![])),
            fee_subsidy: Arc::new(Subsidy::default()),
            native_price_estimator,
            liquidity_order_owners: Default::default(),
            store_computed_fees: false,
        };

        let fee_data = FeeData {
            sell_token: H160::from_low_u64_be(1),
            ..Default::default()
        };
        let (_, valid_to) = fee_estimator
            .compute_subsidized_min_fee(fee_data, Default::default(), Default::default())
            .await
            .unwrap();

        assert_eq!(
            valid_to,
            DateTime::<Utc>::from_utc(NaiveDateTime::from_timestamp(0, 0), Utc)
        );
        assert!(db.0.lock().unwrap().is_empty());
    }

    #[tokio::test]
    async fn applies_fee_factor_to_past_and_new_fees() {
        let sell_token = H160::from_low_u64_be(1);
        let fee_data = FeeData {
            sell_token,
            ..Default::default()
        };
        let native_token_price_estimation_amount = 100.;
        let sell_token_price = 0.1;
        let gas_estimate = 42.;

        let unsubsidized_min_fee = FeeParameters {
            gas_amount: 1337.,
            sell_token_price,
            gas_price: gas_estimate,
        };

        let gas_estimator = Arc::new(FakeGasPriceEstimator(Arc::new(Mutex::new(GasPrice1559 {
            base_fee_per_gas: 0.0,
            max_fee_per_gas: 42.0,
            max_priority_fee_per_gas: 42.0,
        }))));
        let price_estimator = Arc::new(FakePriceEstimator(price_estimation::Estimate {
            out_amount: U256::from_f64_lossy(
                native_token_price_estimation_amount / sell_token_price,
            ),
            gas: 1337,
        }));
        let native_price_estimator = Arc::new(NativePriceEstimator::new(
            price_estimator.clone(),
            Default::default(),
            U256::from_f64_lossy(native_token_price_estimation_amount),
        ));

        let mut measurements = MockMinFeeStoring::new();
        let mut seq = Sequence::new();
        measurements
            .expect_find_measurement_exact()
            .times(1)
            .in_sequence(&mut seq)
            .with(eq(fee_data), always())
            .returning(|_, _| Ok(None));
        measurements
            .expect_save_fee_measurement()
            .times(1)
            .in_sequence(&mut seq)
            .with(eq(fee_data), always(), eq(unsubsidized_min_fee))
            .returning(|_, _, _| Ok(()));
        measurements
            .expect_find_measurement_exact()
            .times(1)
            .in_sequence(&mut seq)
            .with(eq(fee_data), always())
            .returning(move |_, _| Ok(Some(unsubsidized_min_fee)));

        let app_data = AppId([1u8; 32]);
        let fee_estimator = MinFeeCalculator {
            price_estimator,
            gas_estimator,
            measurements: Arc::new(measurements),
            now: Box::new(Utc::now),
            bad_token_detector: Arc::new(ListBasedDetector::deny_list(vec![])),
            fee_subsidy: Arc::new(Subsidy {
                factor: 0.8,
                ..Default::default()
            }),
            native_price_estimator,
            liquidity_order_owners: Default::default(),
            store_computed_fees: true,
        };

        let (fee, _) = fee_estimator
            .compute_subsidized_min_fee(fee_data, app_data, Default::default())
            .await
            .unwrap();
        assert_eq!(
            fee,
            U256::from_f64_lossy(unsubsidized_min_fee.unsubsidized().to_f64_lossy() * 0.8)
        );

        let fee_estimator = MinFeeCalculator {
            fee_subsidy: Arc::new(Subsidy {
                factor: 0.4,
                ..Default::default()
            }),
            ..fee_estimator
        };

        let (fee, _) = fee_estimator
            .compute_subsidized_min_fee(fee_data, Default::default(), Default::default())
            .await
            .unwrap();
        assert_eq!(
            fee,
            U256::from_f64_lossy(unsubsidized_min_fee.unsubsidized().to_f64_lossy() * 0.4)
        );
    }

    #[test]
    fn fee_rounds_up() {
        let fee_data = FeeData {
            sell_token: H160::from_low_u64_be(0),
            buy_token: H160::from_low_u64_be(1),
            ..Default::default()
        };
        let gas_price_estimator =
            Arc::new(FakeGasPriceEstimator(Arc::new(Mutex::new(GasPrice1559 {
                base_fee_per_gas: 0.0,
                max_fee_per_gas: 1.0,
                max_priority_fee_per_gas: 1.0,
            }))));
        let price_estimator = Arc::new(FakePriceEstimator(price_estimation::Estimate {
            out_amount: 1.into(),
            gas: 9,
        }));
        let native_price_estimator = create_default_native_token_estimator(price_estimator.clone());
        let fee_estimator = MinFeeCalculator {
            price_estimator,
            gas_estimator: gas_price_estimator,
            measurements: Arc::new(InMemoryFeeStore::default()),
            now: Box::new(Utc::now),
            bad_token_detector: Arc::new(ListBasedDetector::deny_list(vec![])),
            fee_subsidy: Arc::new(Subsidy {
                factor: 0.5,
                ..Default::default()
            }),
            native_price_estimator,
            liquidity_order_owners: Default::default(),
            store_computed_fees: true,
        };
        let (fee, _) = fee_estimator
            .compute_subsidized_min_fee(fee_data, Default::default(), Default::default())
            .now_or_never()
            .unwrap()
            .unwrap();
        // In floating point the fee would be 4.5 but we always want to round atoms up.
        assert_eq!(fee, 5.into());
        // Fee validates.
        fee_estimator
            .get_unsubsidized_min_fee(fee_data, Default::default(), fee, Default::default())
            .now_or_never()
            .unwrap()
            .unwrap();
    }

    #[test]
    fn no_fees_for_pmms() {
        let liquidity_order_owner = H160([0x42; 20]);
        let fee_estimator = MinFeeCalculator {
            liquidity_order_owners: hashset!(liquidity_order_owner),
            ..MinFeeCalculator::new_for_test(
                Arc::new(FakeGasPriceEstimator(Arc::new(Mutex::new(GasPrice1559 {
                    base_fee_per_gas: 0.0,
                    max_fee_per_gas: 1.0,
                    max_priority_fee_per_gas: 1.0,
                })))),
                Arc::new(FakePriceEstimator(price_estimation::Estimate {
                    out_amount: 1.into(),
                    gas: 9,
                })),
                Box::new(Utc::now),
            )
        };

        let fee_data = FeeData {
            sell_token: H160([1; 20]),
            buy_token: H160([2; 20]),
            ..Default::default()
        };

        assert_eq!(
            fee_estimator
                .compute_subsidized_min_fee(fee_data, AppId::default(), liquidity_order_owner)
                .now_or_never()
                .unwrap()
                .unwrap(),
            (U256::from(0), MAX_DATETIME),
        );
        assert_eq!(
            fee_estimator
                .get_unsubsidized_min_fee(
                    fee_data,
                    AppId::default(),
                    0.into(),
                    liquidity_order_owner
                )
                .now_or_never()
                .unwrap()
                .unwrap(),
            FeeParameters {
                gas_amount: 0.,
                gas_price: 0.,
                sell_token_price: 1.
            },
        );
    }
}
