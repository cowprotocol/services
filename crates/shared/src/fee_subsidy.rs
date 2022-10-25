//! This module contains a `FeeSubsidy` component used for computing fee subsidy
//! parameters for order creation and quoting.
//!
//! Note that this component is designed to return some `Subsidy` parameters
//! instead of a slightly more natural interface:
//! ```text
//! trait FeeSubsidizing {
//!     async fn apply_subsidy(fee_in_eth: f64) -> Result<f64>;
//! }
//! ```
//!
//! While the aforementioned design is more natural and less confusing, it has a
//! downside where it requires the final fee denomincated in native token to be
//! already computed before subsidies can be applied. This means that for
//! quoting, for example, you need to fetch the gas and price estimates *before*
//! you can compute the subsidy.
//!
//! The current design works around this by instead of applying a subsidy on a
//! value directly, it returns some parameters that represent some lower-bounded
//! linear "subsidy function" that can be applied to any value. This allows us
//! to fetch subsidy parameters in parallel with the gas and price estimates,
//! speeding up the overall time required to compute a price quote. The downside
//! is that, if we decide to express subsidies as more complex functions, then
//! the `Subsidy` parameters struct will become more complex, and combining two
//! of them even tricker. In that case, we could change `Subsidy` to be a
//! `Box<dyn Fn(f64) -> f64>` to allow for arbitrary subsidy functions.

pub mod config;
pub mod cow_token;

use anyhow::Result;
use ethcontract::{H160, U256};
use futures::future;
use model::app_id::AppId;
use std::sync::Arc;

#[derive(Clone, Debug, Default)]
pub struct SubsidyParameters {
    /// The trader address.
    pub from: H160,

    /// The app data.
    pub app_data: AppId,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Subsidy {
    /// A flat discount nominated in the native token to discount from fees.
    ///
    /// Flat fee discounts are applied **before** the fee factor.
    pub discount: f64,

    /// Minimum fee amount after applying the flat subsidy. This prevents flat
    /// fee discounts putting the fee amount below 0.
    ///
    /// Flat fee discounts are applied **before** the fee factor.
    pub min_discounted: f64,

    /// An additional fee factor.
    pub factor: f64,
}

impl Default for Subsidy {
    fn default() -> Self {
        Self {
            discount: 0.,
            min_discounted: 0.,
            factor: 1.,
        }
    }
}

/// Trait for describing fee subsidies applied to orders.
#[cfg_attr(test, mockall::automock)]
#[async_trait::async_trait]
pub trait FeeSubsidizing: Send + Sync {
    async fn subsidy(&self, parameters: SubsidyParameters) -> Result<Subsidy>;
}

/// Combine multiple fee subsidy strategies into one!
pub struct FeeSubsidies(pub Vec<Arc<dyn FeeSubsidizing>>);

#[async_trait::async_trait]
impl FeeSubsidizing for FeeSubsidies {
    async fn subsidy(&self, parameters: SubsidyParameters) -> Result<Subsidy> {
        let subsidies = future::try_join_all(
            self.0
                .iter()
                .map(|strategy| strategy.subsidy(parameters.clone())),
        )
        .await?;

        Ok(subsidies
            .into_iter()
            .fold(Subsidy::default(), |a, b| Subsidy {
                discount: a.discount + b.discount,
                // Make sure to take the `max` of the folded min dicounted fee
                // values. This is so we make sure we always respect the "worst"
                // minimum when combining `Subsidy`-ies.
                min_discounted: a.min_discounted.max(b.min_discounted),
                factor: a.factor * b.factor,
            }))
    }
}

// Convenience to allow static subsidies.
#[async_trait::async_trait]
impl FeeSubsidizing for Subsidy {
    async fn subsidy(&self, _: SubsidyParameters) -> Result<Subsidy> {
        Ok(self.clone())
    }
}

/// Everything required to compute the fee amount in sell token
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FeeParameters {
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

impl FeeParameters {
    pub fn unsubsidized(&self) -> U256 {
        self.unsubsidized_with_additional_cost(0u64)
    }

    pub fn unsubsidized_with_additional_cost(&self, additional_cost: u64) -> U256 {
        let fee_in_eth = (self.gas_amount + additional_cost as f64) * self.gas_price;

        dtou(fee_in_eth / self.sell_token_price)
    }

    pub fn subsidized(&self, subsidy: &Subsidy) -> U256 {
        self.subsidized_with_additional_cost(subsidy, 0u64)
    }

    pub fn subsidized_with_additional_cost(&self, subsidy: &Subsidy, additional_cost: u64) -> U256 {
        let fee_in_eth = (self.gas_amount + additional_cost as f64) * self.gas_price;
        let mut discounted_fee_in_eth = fee_in_eth - subsidy.discount;
        if discounted_fee_in_eth < subsidy.min_discounted {
            tracing::warn!(
                %discounted_fee_in_eth, %subsidy.min_discounted,
                "fee after discount below minimum",
            );
            discounted_fee_in_eth = subsidy.min_discounted;
        }

        dtou(discounted_fee_in_eth * subsidy.factor / self.sell_token_price)
    }
}

/// Converts an `f64` to a `U256`.
///
/// We want the conversion from f64 to U256 to use ceil because:
/// 1. For final amounts that end up close to 0 atoms we always take a fee so we
///    are not attackable through low decimal tokens.
/// 2. When validating fees this consistently picks the same amount.
fn dtou(d: f64) -> U256 {
    U256::from_f64_lossy(d.ceil())
}

#[cfg(test)]
mod tests {
    use super::*;

    // Convenience to allow using u32 in tests instead of the struct.
    impl From<u32> for FeeParameters {
        fn from(v: u32) -> Self {
            FeeParameters {
                gas_amount: v as f64,
                gas_price: 1.0,
                sell_token_price: 1.0,
            }
        }
    }

    #[test]
    fn fee_rounds_up() {
        let subsidy = Subsidy {
            factor: 0.5,
            ..Default::default()
        };
        let fee = FeeParameters {
            gas_amount: 9.,
            gas_price: 1.,
            sell_token_price: 1.,
        };

        // In floating point the fee would be 4.5 but we always want to round atoms up.
        assert_eq!(fee.subsidized(&subsidy), 5.into());
    }

    #[test]
    fn apply_fee_factor_capped_at_minimum() {
        let fee = FeeParameters {
            gas_amount: 100_000.,
            gas_price: 1_000_000_000.,
            sell_token_price: 1.,
        };
        let subsidy = Subsidy {
            discount: 500_000_000_000_000.,
            min_discounted: 1_000_000.,
            factor: 0.5,
        };

        assert_eq!(
            fee.subsidized(&subsidy),
            // Note that the fee factor is applied to the minimum discounted fee!
            500_000.into(),
        );
    }

    #[tokio::test]
    async fn combine_multiple_subsidies() {
        let fee_subsidies = FeeSubsidies(vec![
            Arc::new(Subsidy {
                discount: 1e16,
                min_discounted: 1e15,
                factor: 0.9,
            }),
            Arc::new(Subsidy {
                discount: 1e16,
                min_discounted: 1.,
                ..Default::default()
            }),
            Arc::new(Subsidy {
                factor: 0.5,
                ..Default::default()
            }),
        ]);

        assert_eq!(
            fee_subsidies.subsidy(Default::default()).await.unwrap(),
            Subsidy {
                discount: 2e16,
                min_discounted: 1e15,
                factor: 0.45,
            }
        );
    }
}
