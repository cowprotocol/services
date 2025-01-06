//! Module containing an external prices type use for converting amounts to
//! their native asset value.
//!
//! Internally, the `ExternalPrices` keeps a set of exchange rates from tokens
//! to the native asset and assumes the invariant that the exchange rate of the
//! native asset and native wrapped token exist with a value of 1.

use {
    crate::conversions::U256Ext,
    anyhow::{bail, Result},
    ethcontract::{H160, U256},
    model::order::BUY_ETH_ADDRESS,
    num::{BigInt, BigRational, One as _, ToPrimitive as _},
    std::{
        collections::{BTreeMap, HashMap},
        sync::LazyLock,
    },
};

/// A collection of external prices used for converting token amounts to native
/// assets.
#[derive(Clone, Debug)]
pub struct ExternalPrices(HashMap<H160, BigRational>);

impl ExternalPrices {
    /// Creates a new set of external prices for the specified exchange rates.
    pub fn try_new(native_token: H160, mut xrates: HashMap<H160, BigRational>) -> Result<Self> {
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
        Self::try_new(
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
}

impl Default for ExternalPrices {
    fn default() -> Self {
        Self::try_new(Default::default(), Default::default()).unwrap()
    }
}

static UNIT: LazyLock<BigInt> = LazyLock::new(|| BigInt::from(1_000_000_000_000_000_000_u128));

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
mod tests {
    use {
        super::*,
        maplit::{btreemap, hashmap},
    };

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
