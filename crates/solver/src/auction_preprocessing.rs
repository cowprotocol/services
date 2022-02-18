//! Submodule containing helper methods to pre-process auction data before passing it on to the solvers.

use crate::liquidity::LimitOrder;
use anyhow::{bail, Result};
use ethcontract::{H160, U256};
use lazy_static::lazy_static;
use model::order::BUY_ETH_ADDRESS;
use num::{BigInt, BigRational, One as _, ToPrimitive as _};
use shared::conversions::U256Ext as _;
use std::collections::{BTreeMap, HashMap};

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

/// Converts an `Auction` model's price map into an external price map used by
/// the driver for objective value computation.
pub fn to_external_prices(
    prices: BTreeMap<H160, U256>,
    native_token: H160,
) -> Result<HashMap<H160, BigRational>> {
    let mut exchange_rates = prices
        .into_iter()
        .map(|(token, price)| (token, to_native_xrate(price)))
        .collect::<HashMap<_, _>>();

    // Ensure there is a price for the native asset and wrapped native token
    // exist with a value of 1. This protects us from malformed input (in case
    // there are issues with the prices from the `/auction` endpoint for
    // example). Additionally, certain components (like the HTTP solver) expect
    // this price to exist.
    for token in [native_token, BUY_ETH_ADDRESS] {
        match exchange_rates.get(&token) {
            Some(price) if !price.is_one() => {
                let price = price.to_f64().unwrap_or(f64::NAN);
                bail!("malformed native token {:?} price {:?}", token, price);
            }
            Some(_) => {}
            None => {
                exchange_rates.insert(token, BigRational::one());
            }
        }
    }

    Ok(exchange_rates)
}

// vk: I would like to extend this to also check that the order has minimum age but for this we need
// access to the creation date which is a more involved change.
pub fn has_at_least_one_user_order(orders: &[LimitOrder]) -> bool {
    orders.iter().any(|order| !order.is_liquidity_order)
}

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
            to_external_prices(
                btreemap! {
                    H160([1; 20]) => U256::from(100_000_000_000_000_000_u128),
                },
                native_token
            )
            .unwrap(),
            hashmap! {
                H160([1; 20]) => BigRational::new(1.into(), 10.into()),
                native_token => BigRational::one(),
                BUY_ETH_ADDRESS => BigRational::one(),
            },
        );
    }

    #[test]
    fn leaves_malformed_native_token_prices() {
        let native_token = H160([42; 20]);
        assert!(to_external_prices(
            btreemap! {
                native_token => U256::from(4_200_000_000_000_000_000_u128),
            },
            native_token
        )
        .is_err());
        assert!(to_external_prices(
            btreemap! {
                BUY_ETH_ADDRESS => U256::from(13_370_000_000_000_000_000_u128),
            },
            native_token
        )
        .is_err());
    }
}
