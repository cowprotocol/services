use anyhow::Result;
use chrono::{DateTime, Duration, Utc, MAX_DATETIME};
use futures::future::TryFutureExt;
use gas_estimation::GasPriceEstimating;
use model::{app_id::AppId, order::OrderKind};
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
    collections::{HashMap, HashSet},
    sync::{Arc, Mutex},
};

use crate::cow_subsidy::CowSubsidy;

pub type Measurement = (U256, DateTime<Utc>);

/// Fee subsidy configuration.
///
/// Given an estimated fee for a trade, the mimimum fee required for an order is
/// computed using the following formula:
/// ```text
/// (estimated_fee_in_eth - fee_discount) * fee_factor * (partner_additional_fee_factor || 1.0)
/// ```
pub struct FeeSubsidyConfiguration {
    /// A flat discount nominated in the native token to discount from fees.
    ///
    /// Flat fee discounts are applied **before** any multiplicative discounts.
    pub fee_discount: f64,

    /// Minimum fee amount after applying the flat subsidy. This prevents flat
    /// fee discounts putting the fee amount below 0.
    ///
    /// Flat fee discounts are applied **before** any multiplicative discounts.
    pub min_discounted_fee: f64,

    /// A factor to multiply the estimated trading fee with in order to compute
    /// subsidized minimum fee.
    ///
    /// Fee factors are applied **after** flat fee discounts.
    pub fee_factor: f64,

    /// Additional factors per order app ID for computing the subsidized minimum
    /// fee.
    ///
    /// Fee factors are applied **after** flat fee discounts.
    pub partner_additional_fee_factors: HashMap<AppId, f64>,
}

impl Default for FeeSubsidyConfiguration {
    fn default() -> Self {
        Self {
            fee_discount: 0.,
            fee_factor: 1.,
            min_discounted_fee: 0.,
            partner_additional_fee_factors: HashMap::new(),
        }
    }
}

pub struct MinFeeCalculator {
    price_estimator: Arc<dyn PriceEstimating>,
    gas_estimator: Arc<dyn GasPriceEstimating>,
    measurements: Arc<dyn MinFeeStoring>,
    now: Box<dyn Fn() -> DateTime<Utc> + Send + Sync>,
    bad_token_detector: Arc<dyn BadTokenDetecting>,
    fee_subsidy: FeeSubsidyConfiguration,
    native_price_estimator: Arc<dyn NativePriceEstimating>,
    cow_subsidy: Arc<dyn CowSubsidy>,
    liquidity_order_owners: HashSet<H160>,
}

#[derive(Debug, Default, Clone, Copy, Eq, PartialEq)]
pub struct FeeData {
    pub sell_token: H160,
    pub buy_token: H160,
    // For sell orders this is the sell amount before fees.
    pub amount: U256,
    pub kind: OrderKind,
}

/// Everything required to compute the fee amount in sell token
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FeeParameters {
    /// The actual, subsidized fee amount for the quote.
    /// The estimated gas units required to execute the quoted trade.
    pub gas_amount: f64,
    /// The estimated gas price at the time of quoting.
    pub gas_price: f64,
    /// The Ether-denominated price of token at the time of quoting.
    ///
    /// The Ether value of `x` sell tokens is `x * sell_token_price`.
    pub sell_token_price: f64,
}

impl Default for FeeParameters {
    fn default() -> Self {
        Self {
            gas_amount: 0.,
            gas_price: 0.,
            // We can't use `derive(Default)` because then this field would have
            // a value of `0.` and it is used in division. The actual value we
            // use here doesn't really matter as long as its non-zero (since the
            // resulting amount in native token or sell token will be 0
            // regardless), but the multiplicative identity seemed like a
            // natural default value to use.
            sell_token_price: 1.,
        }
    }
}

// We want the conversion from f64 to U256 to use ceil because:
// 1. For final amounts that end up close to 0 atoms we always take a fee so we are not attackable
//    through low decimal tokens.
// 2. When validating fees this consistently picks the same amount.
impl FeeParameters {
    pub fn amount_in_sell_token(&self) -> U256 {
        U256::from_f64_lossy((self.gas_amount * self.gas_price / self.sell_token_price).ceil())
    }

    fn apply_fee_factor(
        &self,
        config: &FeeSubsidyConfiguration,
        app_data: AppId,
        cow_factor: f64,
    ) -> U256 {
        let fee_in_eth = self.gas_amount * self.gas_price;
        let mut discounted_fee_in_eth = fee_in_eth - config.fee_discount;
        if discounted_fee_in_eth < config.min_discounted_fee {
            tracing::warn!(
                "fee after applying fee discount below minimum: {}, capping at {}",
                discounted_fee_in_eth,
                config.min_discounted_fee,
            );
            discounted_fee_in_eth = config.min_discounted_fee;
        }

        let factor = config
            .partner_additional_fee_factors
            .get(&app_data)
            .copied()
            .unwrap_or(1.0)
            * config.fee_factor
            * cow_factor;
        U256::from_f64_lossy((discounted_fee_in_eth * factor / self.sell_token_price).ceil())
    }
}

// Convenience to allow using u32 in tests instead of the struct
#[cfg(test)]
impl From<u32> for FeeParameters {
    fn from(v: u32) -> Self {
        FeeParameters {
            gas_amount: v as f64,
            gas_price: 1.0,
            sell_token_price: 1.0,
        }
    }
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
        fee_subsidy: FeeSubsidyConfiguration,
        native_price_estimator: Arc<dyn NativePriceEstimating>,
        cow_subsidy: Arc<dyn CowSubsidy>,
        liquidity_order_owners: HashSet<H160>,
    ) -> Self {
        Self {
            price_estimator,
            gas_estimator,
            measurements,
            now: Box::new(Utc::now),
            bad_token_detector,
            fee_subsidy,
            native_price_estimator,
            cow_subsidy,
            liquidity_order_owners,
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
        let fee_in_sell_token = fee_parameters.amount_in_sell_token();
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
        let official_valid_until = now + Duration::seconds(STANDARD_VALIDITY_FOR_FEE_IN_SEC);
        let internal_valid_until = now + Duration::seconds(PERSISTED_VALIDITY_FOR_FEE_IN_SEC);

        tracing::debug!(?fee_data, ?app_data, ?user, "computing subsidized fee",);

        let cow_factor = async {
            self.cow_subsidy
                .cow_subsidy_factor(user)
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

                if let Err(err) = self
                    .measurements
                    .save_fee_measurement(fee_data, internal_valid_until, current_fee)
                    .await
                {
                    tracing::warn!(?err, "error saving fee measurement");
                }

                tracing::debug!("using new fee measurement {:?}", current_fee);
                Ok(current_fee)
            }
        };

        let (cow_factor, unsubsidized_min_fee) =
            futures::future::try_join(cow_factor, unsubsidized_min_fee).await?;

        let subsidized_min_fee =
            unsubsidized_min_fee.apply_fee_factor(&self.fee_subsidy, app_data, cow_factor);
        tracing::debug!(
            "computed subsidized fee of {:?}",
            (subsidized_min_fee, fee_data.sell_token),
        );

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

        let cow_factor = self.cow_subsidy.cow_subsidy_factor(user);
        let past_fee = self
            .measurements
            .find_measurement_including_larger_amount(fee_data, (self.now)());
        let (cow_factor, past_fee) = futures::future::try_join(cow_factor, past_fee).await?;
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
            if subsidized_fee >= past_fee.apply_fee_factor(&self.fee_subsidy, app_data, cow_factor)
            {
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
        if subsidized_fee >= current_fee.apply_fee_factor(&self.fee_subsidy, app_data, cow_factor) {
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
            .min_by_key(|estimate| estimate.amount_in_sell_token()))
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
            .min_by_key(|estimate| estimate.amount_in_sell_token()))
    }
}

#[cfg(test)]
#[allow(clippy::float_cmp)]
mod tests {
    use chrono::Duration;
    use futures::FutureExt;
    use gas_estimation::GasPrice1559;
    use maplit::{hashmap, hashset};
    use mockall::{predicate::*, Sequence};
    use shared::{
        bad_token::list_based::ListBasedDetector, gas_price_estimation::FakeGasPriceEstimator,
        price_estimation::mocks::FakePriceEstimator,
        price_estimation::native::NativePriceEstimator,
    };
    use std::sync::Arc;

    use crate::cow_subsidy::FixedCowSubsidy;

    use super::*;

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
                fee_subsidy: Default::default(),
                native_price_estimator: create_default_native_token_estimator(price_estimator),
                cow_subsidy: Arc::new(FixedCowSubsidy::default()),
                liquidity_order_owners: Default::default(),
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
            fee_subsidy: Default::default(),
            native_price_estimator,
            cow_subsidy: Arc::new(FixedCowSubsidy::default()),
            liquidity_order_owners: Default::default(),
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
        let mut fee_estimator = MinFeeCalculator {
            price_estimator,
            gas_estimator: gas_price_estimator,
            measurements: Arc::new(InMemoryFeeStore::default()),
            now: Box::new(Utc::now),
            bad_token_detector: Arc::new(ListBasedDetector::deny_list(vec![])),
            fee_subsidy: FeeSubsidyConfiguration {
                partner_additional_fee_factors: hashmap! { app_data => 0.5 },
                ..Default::default()
            },
            native_price_estimator,
            cow_subsidy: Arc::new(FixedCowSubsidy(0.5)),
            liquidity_order_owners: Default::default(),
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
                .amount_in_sell_token(),
            fee * 4
        );
        assert!(fee_estimator
            .get_unsubsidized_min_fee(fee_data, Default::default(), fee, user)
            .await
            .is_err());
        let lower_fee = fee - 1;
        assert!(fee_estimator
            .get_unsubsidized_min_fee(fee_data, app_data, lower_fee, user)
            .await
            .is_err());

        // repeat without user so no extra cow subsidy
        fee_estimator.cow_subsidy = Arc::new(FixedCowSubsidy(1.0));
        let (fee_2, _) = fee_estimator
            .compute_subsidized_min_fee(fee_data, app_data, user)
            .await
            .unwrap();
        assert_eq!(fee_2, fee * 2);
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
            fee_subsidy: FeeSubsidyConfiguration {
                fee_factor: 0.8,
                partner_additional_fee_factors: hashmap! { app_data => 0.5 },
                ..Default::default()
            },
            native_price_estimator,
            cow_subsidy: Arc::new(FixedCowSubsidy::default()),
            liquidity_order_owners: Default::default(),
        };

        let (fee, _) = fee_estimator
            .compute_subsidized_min_fee(fee_data, app_data, Default::default())
            .await
            .unwrap();
        assert_eq!(
            fee,
            U256::from_f64_lossy(
                unsubsidized_min_fee.amount_in_sell_token().to_f64_lossy() * 0.8 * 0.5
            )
        );

        let (fee, _) = fee_estimator
            .compute_subsidized_min_fee(fee_data, Default::default(), Default::default())
            .await
            .unwrap();
        assert_eq!(
            fee,
            U256::from_f64_lossy(unsubsidized_min_fee.amount_in_sell_token().to_f64_lossy() * 0.8)
        );
    }

    #[test]
    fn test_apply_fee_factor_capped_at_minimum() {
        let unsubsidized = FeeParameters {
            gas_amount: 100_000.,
            gas_price: 1_000_000_000.,
            sell_token_price: 1.,
        };

        let fee_configuration = FeeSubsidyConfiguration {
            fee_discount: 500_000_000_000_000.,
            min_discounted_fee: 1_000_000.,
            fee_factor: 0.5,
            ..Default::default()
        };

        assert_eq!(
            unsubsidized.apply_fee_factor(&fee_configuration, Default::default(), 1.0),
            // Note that the fee factor is applied to the minimum discounted fee!
            500_000.into(),
        );
    }

    #[test]
    fn test_apply_fee_factor_order() {
        let unsubsidized = FeeParameters {
            gas_amount: 100_000.,
            gas_price: 1_000_000_000.,
            sell_token_price: 1.,
        };

        let app_id = AppId([1u8; 32]);
        let fee_configuration = FeeSubsidyConfiguration {
            fee_discount: 50_000_000_000_000.,
            fee_factor: 0.5,
            min_discounted_fee: 0.,
            partner_additional_fee_factors: maplit::hashmap! {
                app_id => 0.1,
            },
        };

        // (100G - 50G) * 0.5
        assert_eq!(
            unsubsidized.apply_fee_factor(&fee_configuration, Default::default(), 1.0),
            25_000_000_000_000u64.into()
        );
        // Additionally multiply with 0.1 if partner app id is used
        assert_eq!(
            unsubsidized.apply_fee_factor(&fee_configuration, app_id, 1.0),
            2_500_000_000_000u64.into()
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
            fee_subsidy: FeeSubsidyConfiguration {
                fee_factor: 0.5,
                ..Default::default()
            },
            native_price_estimator,
            cow_subsidy: Arc::new(FixedCowSubsidy::default()),
            liquidity_order_owners: Default::default(),
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
