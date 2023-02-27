//! Implementation of CIP-14 risk adjusted solver rewards as described in https://forum.cow.fi/t/cip-14-risk-adjusted-solver-rewards/1132 .
//!
//! Note slight differences in the formulas due conversion of units (gas, gas
//! price, COW to ETH) that are glossed over in the CIP.
//!
//! This module uses argument structs in order to better document and
//! differentiate many arguments of the type f64 where it would be easy to mix
//! them up when calling the function.

use {
    crate::database::Postgres,
    anyhow::{ensure, Context, Result},
    database::orders::Quote,
    futures::StreamExt,
    gas_estimation::GasPriceEstimating,
    model::order::{Order, OrderClass, OrderUid},
    primitive_types::H160,
    shared::price_estimation::native::{NativePriceEstimateResult, NativePriceEstimating},
    std::{
        sync::{Arc, Mutex},
        time::{Duration, Instant},
    },
};

#[derive(Copy, Clone, Debug, Default)]
pub struct Configuration {
    pub beta: f64,
    pub alpha1: f64,
    pub alpha2: f64,
    /// T, in COW base units
    pub profit: f64,
    /// in gas units
    pub gas_cap: f64,
    /// in COW base units
    pub reward_cap: f64,
}

pub struct Calculator {
    config: Configuration,
    database: Postgres,
    gas_price: Arc<dyn GasPriceEstimating>,
    native_price: BestEffortCowPriceEstimator,
}

impl Calculator {
    /// Time limit for CoW token price cached in [BestEffortCowPriceEstimator].
    /// Hardcoded to avoid bloating the service configuration.
    const COW_PRICE_MAX_AGE: Duration = Duration::from_secs(10 * 60);

    pub fn new(
        config: Configuration,
        database: Postgres,
        cow_token: H160,
        gas_price: Arc<dyn GasPriceEstimating>,
        native_price: Arc<dyn NativePriceEstimating>,
    ) -> Self {
        Self {
            config,
            database,
            gas_price,
            native_price: BestEffortCowPriceEstimator::new(
                native_price,
                cow_token,
                Self::COW_PRICE_MAX_AGE,
            ),
        }
    }

    /// Returns the rewards in COW base units for several orders.
    ///
    /// An outer error indicates that no reward calculations were performed.
    ///
    /// An inner error indicates that the reward for this order could not be
    /// calculated.
    ///
    /// The Ok-Vec has the same number of elements as the input orders-slice.
    /// Each element corresponds to the order at the same index.
    pub async fn calculate_many(&self, orders: &[Order]) -> Result<Vec<Result<f64>>> {
        if orders.is_empty() {
            return Ok(Vec::new());
        }
        let gas_price = async {
            Ok(self
                .gas_price
                .estimate()
                .await
                .context("gas price")?
                .effective_gas_price())
        };
        let cow_price = async {
            self.native_price
                .get_price(Instant::now())
                .await
                .context("cow native price")
        };
        let (gas_price, cow_price) = futures::future::try_join(gas_price, cow_price).await?;
        tracing::trace!("gas_price={gas_price:.2e} cow_price={cow_price:.2e}");
        Ok(futures::stream::iter(orders)
            .then(|order| async {
                if order.metadata.class == OrderClass::Liquidity || order.data.partially_fillable {
                    return Ok(0.);
                }
                let quote = self
                    .quote(&order.metadata.uid)
                    .await?
                    .context("missing quote")?;
                capped_reward_cow(
                    self.config,
                    CappedCowArgs {
                        gas: quote.gas_amount,
                        gas_price,
                        cow_price,
                    },
                )
            })
            .collect()
            .await)
    }

    async fn quote(&self, order: &OrderUid) -> Result<Option<Quote>, sqlx::Error> {
        let mut ex = self.database.0.acquire().await?;
        database::orders::read_quote(&mut ex, &database::byte_array::ByteArray(order.0)).await
    }
}

const COW_BASE: f64 = 1e18;

#[derive(Copy, Clone, Debug)]
struct CappedCowArgs {
    /// units of gas needed to settle the order
    gas: f64,
    /// the price of one gas unit in ETH atoms
    gas_price: f64,
    /// the price of one COW atom in ETH atoms
    cow_price: f64,
}

fn capped_reward_cow(
    Configuration {
        beta,
        alpha1,
        alpha2,
        profit,
        gas_cap,
        reward_cap,
    }: Configuration,
    CappedCowArgs {
        gas,
        gas_price,
        cow_price,
    }: CappedCowArgs,
) -> Result<f64> {
    let args = UncappedEthArgs {
        beta,
        alpha1,
        alpha2,
        profit: profit * COW_BASE * cow_price,
        gas: gas.min(gas_cap),
        gas_price,
    };
    let reward = uncapped_reward_eth_atoms(args) / cow_price / COW_BASE;
    ensure!(
        reward.is_finite() && reward >= 0.,
        "reward is weird {:?}",
        reward
    );
    Ok(reward.min(reward_cap))
}

#[derive(Copy, Clone, Debug)]
struct UncappedEthArgs {
    /// β
    beta: f64,
    /// ɑ1
    alpha1: f64,
    /// ɑ2
    alpha2: f64,
    /// T, in ETH atoms, conversion to and from COW has to be done outside
    profit: f64,
    /// units of gas needed to settle the order, capping has to be done outside
    gas: f64,
    /// the price of one gas unit in ETH atoms
    gas_price: f64,
}

/// Returns uncapped reward in ETH atoms.
fn uncapped_reward_eth_atoms(
    UncappedEthArgs {
        beta,
        alpha1,
        alpha2,
        profit,
        gas,
        gas_price,
    }: UncappedEthArgs,
) -> f64 {
    let cost = gas * gas_price;
    // The way https://github.com/cowprotocol/risk_adjusted_rewards calculates the parameters gas is
    // expressed in thousandth and gas price in gwei so we need to adjust our atom
    // based values.
    let exponent = -beta - alpha1 * (gas / 1e3) - alpha2 * (gas_price / 1e9);
    let revert_probability = 1. / (1. + exponent.exp());
    (profit + cost) / (1. - revert_probability) - cost
}

/// A caching wrapper over [NativePriceEstimating] used to estimate and cache
/// CoW token price. Implemented to enable the [Calculator] produce rewards even
/// when a fresh estimate is not available.
struct BestEffortCowPriceEstimator {
    inner: Arc<dyn NativePriceEstimating>,
    cow_token: H160,
    /// Cached value and a timestamp.
    fallback_cache: Mutex<Option<(f64, Instant)>>,
    /// How long is the value valid after being set.
    max_age: Duration,
}

impl BestEffortCowPriceEstimator {
    fn new(inner: Arc<dyn NativePriceEstimating>, cow_token: H160, max_age: Duration) -> Self {
        Self {
            inner,
            cow_token,
            fallback_cache: Mutex::new(None),
            max_age,
        }
    }

    /// Attempts to use the inner estimator to get a fresh price estimate.
    /// If that fails, attempts to return the value from cache.
    async fn get_price(&self, current_time: Instant) -> NativePriceEstimateResult {
        let fresh = self
            .inner
            .estimate_native_prices(std::slice::from_ref(&self.cow_token))
            .next()
            .await
            .unwrap()
            .1;

        match fresh {
            Ok(price) => {
                self.set_cached(price, current_time);
                Ok(price)
            }
            Err(error) => {
                let price = self.get_cached(current_time);
                if let Some(price) = price {
                    tracing::warn!(
                        ?error,
                        "Using an old CoW token price from a fallback cache, fetching a fresh \
                         estimate failed"
                    );
                    Ok(price)
                } else {
                    Err(error)
                }
            }
        }
    }

    fn get_cached(&self, current_time: Instant) -> Option<f64> {
        let (price, timestamp) = (*self.fallback_cache.lock().unwrap())?;

        if timestamp + self.max_age >= current_time {
            Some(price)
        } else {
            None
        }
    }

    fn set_cached(&self, price: f64, current_time: Instant) {
        self.fallback_cache
            .lock()
            .unwrap()
            .replace((price, current_time));
    }
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        approx::assert_relative_eq,
        shared::price_estimation::{
            mocks::{FailingPriceEstimator, FakePriceEstimator},
            native::NativePriceEstimator,
            Estimate,
        },
    };

    // realistic values
    const CONFIG: Configuration = Configuration {
        beta: -4.321187333208046,
        alpha1: 0.0018180663326599151,
        alpha2: 0.005331921562999044,
        profit: 37.,
        gas_cap: 1.25e6,
        reward_cap: 2.5e3,
    };
    const COW_PRICE_IN_ETH: f64 = 6.46e-5;

    #[test]
    fn artificial() {
        // revert prob 0.5, cost 4, reward has to be equal to cost because 0 profit
        let args = UncappedEthArgs {
            beta: 0.,
            alpha1: 0.,
            alpha2: 0.,
            profit: 0.,
            gas: 2.,
            gas_price: 2.,
        };
        assert_eq!(uncapped_reward_eth_atoms(args), 4.);

        // Now we want on average 1 profit. Reward is only paid out on success so has to
        // be doubled to account for 0.5 prob.
        let args = UncappedEthArgs {
            beta: 0.,
            alpha1: 0.,
            alpha2: 0.,
            profit: 1.,
            gas: 2.,
            gas_price: 2.,
        };
        assert_eq!(uncapped_reward_eth_atoms(args), 6.);
    }

    #[test]
    fn realistic() {
        let mut args = UncappedEthArgs {
            beta: CONFIG.beta,
            alpha1: CONFIG.alpha1,
            alpha2: CONFIG.alpha2,
            profit: 0.,
            gas: 500e3,
            gas_price: 20e9,
        };
        // cost = 500e3 * 20e9 = 0.01 ETH
        // revert probability ~= 0.035
        // reward ~= 0.000366 ETH
        // It takes 0.000366 ETH to make up for the expected revert cost.
        let reward = uncapped_reward_eth_atoms(args);
        assert_relative_eq!(reward, 3.66e14, max_relative = 0.01);

        // Include the target COW reward. This is significantly more than the revert
        // cost so the reward goes to ~0.00284 ETH.
        args.profit = CONFIG.profit * COW_BASE * COW_PRICE_IN_ETH;
        let reward_eth = uncapped_reward_eth_atoms(args);
        assert_relative_eq!(reward_eth, 2.84e15, max_relative = 0.01);

        // Same parameters but with conversion to COW. The equivalent COW amount to the
        // previous ETH is 44.
        let args = CappedCowArgs {
            gas: args.gas,
            gas_price: args.gas_price,
            cow_price: COW_PRICE_IN_ETH,
        };
        let reward = capped_reward_cow(CONFIG, args).unwrap();
        assert_relative_eq!(reward, 44.0, max_relative = 0.01);
    }

    #[test]
    fn caps_gas() {
        let mut args = CappedCowArgs {
            gas: CONFIG.gas_cap,
            gas_price: 1.,
            cow_price: 1.,
        };
        let r0 = capped_reward_cow(CONFIG, args).unwrap();
        // Haven't hit reward cap yet but increasing gas use doesn't increase reward.
        assert!(r0 < CONFIG.reward_cap);
        args.gas *= 100.;
        let r1 = capped_reward_cow(CONFIG, args).unwrap();
        assert_eq!(r0, r1);
    }

    #[test]
    fn caps_reward() {
        // realistic gas and gas price, low cow price
        let mut args = CappedCowArgs {
            gas: 500e3,
            gas_price: 20e9,
            cow_price: 1e-12,
        };
        let r0 = capped_reward_cow(CONFIG, args).unwrap();
        // Despite gas being below cap we hit the maximum reward and increasing gas
        // doesn't increase reward.
        assert!(args.gas < CONFIG.gas_cap);
        assert_eq!(r0, CONFIG.reward_cap);
        args.gas *= 100.;
        let r1 = capped_reward_cow(CONFIG, args).unwrap();
        assert_eq!(r0, r1);
    }

    #[tokio::test]
    #[ignore]
    async fn mainnet() {
        shared::tracing::initialize_reentrant("autopilot=trace");
        let db = Postgres::new(&std::env::var("DB_URL").unwrap())
            .await
            .unwrap();
        let calc = Calculator::new(
            CONFIG,
            db.clone(),
            testlib::tokens::COW,
            Arc::new(gas_estimation::GasNowGasStation::new(
                shared::gas_price_estimation::Client(Default::default()),
            )),
            Arc::new(shared::price_estimation::native::NativePriceEstimator::new(
                Arc::new(shared::price_estimation::zeroex::ZeroExPriceEstimator::new(
                    Arc::new(shared::zeroex_api::DefaultZeroExApi::with_default_url(
                        Default::default(),
                    )),
                    Default::default(),
                    shared::rate_limiter::RateLimiter::test(),
                    testlib::protocol::SETTLEMENT,
                )),
                testlib::tokens::WETH,
                primitive_types::U256::from_f64_lossy(1e18),
            )),
        );

        let min_valid_to = model::time::now_in_epoch_seconds();
        let orders = db
            .solvable_orders(min_valid_to, Default::default())
            .await
            .unwrap()
            .orders;
        let results = calc.calculate_many(&orders).await.unwrap();
        assert!(orders.len() == results.len());
        for (order, result) in orders.iter().zip(results) {
            println!("{} {:?}", order.metadata.uid, result);
        }
    }

    #[tokio::test]
    async fn best_effort_estimator() {
        let fake_estimator = Arc::new(NativePriceEstimator::new(
            Arc::new(FakePriceEstimator(Estimate {
                out_amount: 20.into(),
                gas: 100,
            })),
            [0; 20].into(),
            10.into(),
        ));
        let failing_estimator = Arc::new(NativePriceEstimator::new(
            Arc::new(FailingPriceEstimator),
            [0; 20].into(),
            10.into(),
        ));
        let mut estimator = BestEffortCowPriceEstimator::new(
            failing_estimator.clone(),
            [1; 20].into(),
            Duration::from_millis(1000),
        );

        assert!(estimator.get_price(Instant::now()).await.is_err());

        estimator.inner = fake_estimator.clone();
        let cache_set_at = Instant::now();
        let estimate = estimator.get_price(cache_set_at).await.unwrap();

        estimator.inner = failing_estimator;
        assert_eq!(estimator.get_price(cache_set_at).await.unwrap(), estimate);
        assert_eq!(
            estimator
                .get_price(cache_set_at + Duration::from_millis(500))
                .await
                .unwrap(),
            estimate
        );
        assert_eq!(
            estimator
                .get_price(cache_set_at + Duration::from_millis(1000))
                .await
                .unwrap(),
            estimate
        );
        assert!(estimator
            .get_price(cache_set_at + Duration::from_millis(1001))
            .await
            .is_err());

        estimator.inner = fake_estimator;
        assert_eq!(
            estimator
                .get_price(cache_set_at + Duration::from_millis(1002))
                .await
                .unwrap(),
            estimate
        );
    }
}
