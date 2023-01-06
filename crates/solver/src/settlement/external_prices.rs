//! Module containing an external prices type use for converting amounts to
//! their native asset value.
//!
//! Internally, the `ExternalPrices` keeps a set of exchange rates from tokens
//! to the native asset and assumes the invariant that the exchange rate of the
//! native asset and native wrapped token exist with a value of 1.

use anyhow::{bail, Result};
use ethcontract::{H160, U256};
use lazy_static::lazy_static;
use model::order::BUY_ETH_ADDRESS;
use num::{BigInt, BigRational, One as _, ToPrimitive as _};
use shared::conversions::U256Ext as _;
use std::collections::{BTreeMap, HashMap};

/// A collection of external prices used for converting token amounts to native
/// assets.
#[derive(Clone, Debug)]
pub struct ExternalPrices(HashMap<H160, BigRational>);

impl ExternalPrices {
    /// Creates a new set of external prices for the specified exchange rates.
    pub fn new(native_token: H160, mut xrates: HashMap<H160, BigRational>) -> Result<Self> {
        // Make sure to verify our invariant that native asset price and native
        // wrapped asset price exist with a value of 1. This protects us from
        // malformed input (in case there are issues with the prices from the
        // `/auction` endpoint for example).
        for token in [native_token, BUY_ETH_ADDRESS] {
            match xrates.get(&token) {
                Some(price) if !price.is_one() => {
                    let price = price.to_f64().unwrap_or(f64::NAN);
                    bail!("malformed native token {:?} price {:.2e}", token, price);
                }
                Some(_) => {}
                None => {
                    xrates.insert(token, BigRational::one());
                }
            }
        }

        Ok(Self(xrates))
    }

    /// Returns a set of external prices for the specified auction model prices.
    pub fn try_from_auction_prices(
        native_token: H160,
        prices: BTreeMap<H160, U256>,
    ) -> Result<Self> {
        Self::new(
            native_token,
            prices
                .into_iter()
                .map(|(token, price)| (token, to_native_xrate(price)))
                .collect(),
        )
    }

    /// Returns the price of a token relative to the native token.
    /// I.e., the price of the native token is 1 and
    /// the price of a token T is represented as how much native token
    // is needed in order to buy 1 atom of the token T
    pub fn price(&self, token: &H160) -> Option<&BigRational> {
        self.0.get(token)
    }

    /// Converts a token amount into its native asset equivalent.
    ///
    /// # Panic
    ///
    /// This method panics if the specified token does not have a price.
    pub fn get_native_amount(&self, token: H160, amount: BigRational) -> BigRational {
        self.try_get_native_amount(token, amount)
            .unwrap_or_else(|| panic!("missing price for {token}"))
    }

    /// Converts a token amount into its native asset equivalent.
    ///
    /// This method is similar to [`get_native_amount`] except that it will
    /// return `None` if the specified token does not have a price instead of
    /// panicking.
    pub fn try_get_native_amount(&self, token: H160, amount: BigRational) -> Option<BigRational> {
        Some(self.0.get(&token)? * amount)
    }

    /// Converts a set of external prices into prices for the HTTP solver.
    ///
    /// Specifically the HTTP solver expects prices to be in `f64` and there not
    /// to be an entry for `BUY_ETH_ADDRESS` since orders are already normalized
    /// to use the native wrapped token.
    pub fn into_http_solver_prices(&self) -> HashMap<H160, f64> {
        let mut prices = self
            .0
            .iter()
            .filter_map(|(token, price)| Some((*token, price.to_f64()?)))
            .collect::<HashMap<H160, f64>>();
        prices.remove(&BUY_ETH_ADDRESS);
        prices
    }
}

impl Default for ExternalPrices {
    fn default() -> Self {
        Self::new(Default::default(), Default::default()).unwrap()
    }
}

lazy_static! {
    static ref UNIT: BigInt = BigInt::from(1_000_000_000_000_000_000_u128);
}

/// Converts a token price from the orderbook API `/auction` endpoint to an
/// native token exchange rate.
fn to_native_xrate(price: U256) -> BigRational {
    // Prices returned by the API are already denominated in native token with
    // 18 decimals. This means, its value corresponds to how much native token
    // is needed in order to buy 1e18 of the priced token.
    // Thus, in order to compute an exchange rate from the priced token to the
    // native token we simply need to compute `price / 1e18`. This results in
    // an exchange rate such that `x TOKEN * xrate = y ETH`.
    price.to_big_rational() / &*UNIT
}

#[cfg(test)]
/// Macro for instantiating an `ExternalPrices` instance for testing.
macro_rules! externalprices {
    (native_token: $nt:expr $(, $($t:tt)*)?) => {
        $crate::settlement::external_prices::ExternalPrices::new(
            $nt,
            ::maplit::hashmap!($($($t)*)*),
        )
        .unwrap()
    };
}
#[cfg(test)]
pub(crate) use externalprices;

#[cfg(test)]
mod tests {
    use super::*;
    use maplit::{btreemap, hashmap};

    #[test]
    fn converts_prices_to_exchange_rates() {
        // By definition, the price of ETH is 1e18 and its xrate is 1.
        let eth_price = U256::from(1_000_000_000_000_000_000_u128);
        assert_eq!(to_native_xrate(eth_price), BigRational::one());

        // GNO is typically traded at around Ξ0.1. With the price
        // representation we use here, this would be 1e17.
        let gno_price = U256::from_f64_lossy(1e17);
        let gno_xrate = to_native_xrate(gno_price);
        assert_eq!(
            gno_xrate,
            BigRational::new(BigInt::from(1), BigInt::from(10))
        );

        // 1000 GNO is worth roughly 100 ETH
        let gno_amount = BigInt::from(1000) * &*UNIT;
        let eth_amount = gno_xrate * gno_amount;
        assert_eq!(
            eth_amount,
            BigRational::from_integer(BigInt::from(100) * &*UNIT)
        );
    }

    #[test]
    fn augments_price_map_with_native_token_prices() {
        let native_token = H160([42; 20]);
        assert_eq!(
            ExternalPrices::try_from_auction_prices(
                native_token,
                btreemap! {
                    H160([1; 20]) => U256::from(100_000_000_000_000_000_u128),
                },
            )
            .unwrap()
            .0,
            hashmap! {
                H160([1; 20]) => BigRational::new(1.into(), 10.into()),
                native_token => BigRational::one(),
                BUY_ETH_ADDRESS => BigRational::one(),
            },
        );
    }

    #[test]
    fn from_auction_price_errors_on_invalid_native_prices() {
        let native_token = H160([42; 20]);
        assert!(ExternalPrices::try_from_auction_prices(
            native_token,
            btreemap! {
                native_token => U256::from(4_200_000_000_000_000_000_u128),
            },
        )
        .is_err());
        assert!(ExternalPrices::try_from_auction_prices(
            native_token,
            btreemap! {
                BUY_ETH_ADDRESS => U256::from(13_370_000_000_000_000_000_u128),
            },
        )
        .is_err());
    }
}
