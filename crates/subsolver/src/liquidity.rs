use {
    alloy_primitives::{Address, U256},
    solvers::domain::{eth, liquidity},
    std::collections::HashSet,
};

/// Converts a set of orders into the token pairs we need to fetch liquidity
/// for.
pub fn token_pairs_from_orders(
    orders: &[crate::orderbook::Order],
    base_tokens: &[Address],
) -> HashSet<(Address, Address)> {
    let mut pairs = HashSet::new();
    for order in orders {
        // Direct pair
        pairs.insert(ordered_pair(order.sell_token, order.buy_token));

        // Via base tokens (for multi-hop routing)
        for &base in base_tokens {
            if base != order.sell_token {
                pairs.insert(ordered_pair(order.sell_token, base));
            }
            if base != order.buy_token {
                pairs.insert(ordered_pair(order.buy_token, base));
            }
        }
    }
    pairs
}

fn ordered_pair(a: Address, b: Address) -> (Address, Address) {
    if a < b { (a, b) } else { (b, a) }
}

/// Creates a domain liquidity entry from a Uniswap V2 pool.
pub fn constant_product_liquidity(
    pool_address: Address,
    token_a: Address,
    reserve_a: U256,
    token_b: Address,
    reserve_b: U256,
) -> Option<liquidity::Liquidity> {
    let asset_a = eth::Asset {
        token: eth::TokenAddress(token_a),
        amount: reserve_a,
    };
    let asset_b = eth::Asset {
        token: eth::TokenAddress(token_b),
        amount: reserve_b,
    };
    let reserves = liquidity::constant_product::Reserves::new(asset_a, asset_b)?;

    Some(liquidity::Liquidity {
        id: liquidity::Id(format!("{pool_address:?}")),
        address: pool_address,
        gas: eth::Gas(U256::from(90_171u64)),
        state: liquidity::State::ConstantProduct(liquidity::constant_product::Pool {
            reserves,
            fee: eth::Rational::new_raw(U256::from(3), U256::from(1000)),
        }),
    })
}
