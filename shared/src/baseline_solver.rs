use ethcontract::{H160, U256};
use model::TokenPair;
use num::BigRational;
use std::collections::{HashMap, HashSet};

use crate::pool_fetching::Pool;

type PathCandidate = Vec<H160>;

// Given a path and sell amount (first token of the path) estimates the buy amount (last token of the path).
// Returns None if the path is invalid or pool information doesn't exist.
pub fn estimate_buy_amount(
    sell_amount: U256,
    path: &[H160],
    pools: &HashMap<TokenPair, Pool>,
) -> Option<U256> {
    let sell_token = path.first()?;
    path.iter()
        .skip(1)
        .fold(Some((sell_amount, *sell_token)), |previous, current| {
            let previous = match previous {
                Some(previous) => previous,
                None => return None,
            };

            match pools.get(&TokenPair::new(*current, previous.1)?) {
                Some(pool) => pool.get_amount_out(previous.1, previous.0),
                None => None,
            }
        })
        .map(|(amount, _)| amount)
}

// Given a path and buy amount (last token of the path) estimates the sell amount (first token of the path).
// Returns None if the path is invalid or pool information doesn't exist.
pub fn estimate_sell_amount(
    buy_amount: U256,
    path: &[H160],
    pools: &HashMap<TokenPair, Pool>,
) -> Option<U256> {
    let buy_token = path.last()?;
    path.iter()
        .rev()
        .skip(1)
        .fold(Some((buy_amount, *buy_token)), |previous, current| {
            let previous = match previous {
                Some(previous) => previous,
                None => return None,
            };
            match pools.get(&TokenPair::new(*current, previous.1)?) {
                Some(pool) => pool.get_amount_in(previous.1, previous.0),
                None => None,
            }
        })
        .map(|(amount, _)| amount)
}

pub fn estimate_spot_price(path: &[H160], pools: &HashMap<TokenPair, Pool>) -> Option<BigRational> {
    let sell_token = path.first()?;
    path.iter()
        .skip(1)
        .fold(
            Some((BigRational::from_integer(1.into()), *sell_token)),
            |previous, current| {
                let previous = match previous {
                    Some(previous) => previous,
                    None => return None,
                };
                let pool = pools.get(&TokenPair::new(*current, previous.1)?)?;
                let (price, token) = pool.get_spot_price(previous.1)?;
                Some((previous.0 * price, token))
            },
        )
        .map(|(amount, _)| amount)
}

// Returns possible paths from sell_token to buy token, given a list of potential intermediate base tokens
// and a maximum number of intermediate steps.
pub fn path_candidates(
    sell_token: H160,
    buy_token: H160,
    base_tokens: &HashSet<H160>,
    max_hops: usize,
) -> HashSet<PathCandidate> {
    let mut candidates = HashSet::new();

    // Start with just the sell token (yields the direct pair candidate in the 0th iteration)
    let mut path_prefixes = vec![vec![sell_token]];
    for _ in 0..(max_hops + 1) {
        let mut next_round_path_prefixes = vec![];
        for path_prefix in &path_prefixes {
            // For this round, add the buy token and path to the candidates
            let mut full_path = path_prefix.clone();
            full_path.push(buy_token);
            candidates.insert(full_path);

            // For the next round, amend current prefix with all base tokens that are not yet on the path
            for base_token in base_tokens {
                if base_token != &buy_token && !path_prefix.contains(base_token) {
                    let mut next_round_path_prefix = path_prefix.clone();
                    next_round_path_prefix.push(*base_token);
                    next_round_path_prefixes.push(next_round_path_prefix);
                }
            }
        }
        path_prefixes = next_round_path_prefixes;
    }
    candidates
}

pub fn token_path_to_pair_path(token_list: &[H160]) -> Vec<TokenPair> {
    token_list
        .windows(2)
        .map(|window| {
            TokenPair::new(window[0], window[1]).expect("token list contains same token in a row")
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::conversions::big_rational_to_float;
    use ethcontract::H160;
    use maplit::{hashmap, hashset};
    use model::TokenPair;
    use std::iter::FromIterator;

    #[test]
    fn test_path_candidates() {
        let base_tokens = vec![
            H160::from_low_u64_be(0),
            H160::from_low_u64_be(1),
            H160::from_low_u64_be(2),
        ];
        let base_token_set = &HashSet::from_iter(base_tokens.clone());

        let sell_token = H160::from_low_u64_be(4);
        let buy_token = H160::from_low_u64_be(5);

        // 0 hops
        assert_eq!(
            path_candidates(sell_token, buy_token, base_token_set, 0),
            hashset! {vec![sell_token, buy_token]}
        );

        // 1 hop with all permutations
        assert_eq!(
            path_candidates(sell_token, buy_token, base_token_set, 1),
            hashset! {
                vec![sell_token, buy_token],
                vec![sell_token, base_tokens[0], buy_token],
                vec![sell_token, base_tokens[1], buy_token],
                vec![sell_token, base_tokens[2], buy_token],

            }
        );

        // 2 & 3 hops check count
        assert_eq!(
            path_candidates(sell_token, buy_token, base_token_set, 2).len(),
            10
        );
        assert_eq!(
            path_candidates(sell_token, buy_token, base_token_set, 3).len(),
            16
        );

        // 4 hops should not yield any more permutations since we used all base tokens
        assert_eq!(
            path_candidates(sell_token, buy_token, base_token_set, 4).len(),
            16
        );

        // Ignores base token if part of buy or sell
        assert_eq!(
            path_candidates(base_tokens[0], buy_token, base_token_set, 1),
            hashset! {
                vec![base_tokens[0], buy_token],
                vec![base_tokens[0], base_tokens[1], buy_token],
                vec![base_tokens[0], base_tokens[2], buy_token],

            }
        );
        assert_eq!(
            path_candidates(sell_token, base_tokens[0], base_token_set, 1),
            hashset! {
                vec![sell_token, base_tokens[0]],
                vec![sell_token, base_tokens[1], base_tokens[0]],
                vec![sell_token, base_tokens[2], base_tokens[0]],

            }
        );
    }

    #[test]
    fn test_estimate_amount_returns_none_if_it_contains_pair_without_pool() {
        let sell_token = H160::from_low_u64_be(1);
        let intermediate = H160::from_low_u64_be(2);
        let buy_token = H160::from_low_u64_be(3);

        let path = vec![sell_token, intermediate, buy_token];
        let pools = [Pool::uniswap(
            TokenPair::new(path[0], path[1]).unwrap(),
            (100, 100),
        )];
        let pools = hashmap! {
            pools[0].tokens => pools[0].clone(),
        };

        assert_eq!(estimate_buy_amount(1.into(), &path, &pools), None);
        assert_eq!(estimate_sell_amount(1.into(), &path, &pools), None);
    }

    #[test]
    fn test_estimate_amount() {
        let sell_token = H160::from_low_u64_be(1);
        let intermediate = H160::from_low_u64_be(2);
        let buy_token = H160::from_low_u64_be(3);

        let path = vec![sell_token, intermediate, buy_token];
        let pools = [
            Pool::uniswap(TokenPair::new(path[0], path[1]).unwrap(), (100, 100)),
            Pool::uniswap(TokenPair::new(path[1], path[2]).unwrap(), (200, 50)),
        ];
        let pools = hashmap! {
            pools[0].tokens => pools[0].clone(),
            pools[1].tokens => pools[1].clone(),
        };

        assert_eq!(
            estimate_buy_amount(10.into(), &path, &pools),
            Some(2.into())
        );

        assert_eq!(
            estimate_sell_amount(10.into(), &path, &pools),
            Some(105.into())
        );
    }

    #[test]
    fn test_estimate_sell_amount_returns_none_buying_too_much() {
        let sell_token = H160::from_low_u64_be(1);
        let intermediate = H160::from_low_u64_be(2);
        let buy_token = H160::from_low_u64_be(3);

        let path = vec![sell_token, intermediate, buy_token];
        let pools = [
            Pool::uniswap(TokenPair::new(path[0], path[1]).unwrap(), (100, 100)),
            Pool::uniswap(TokenPair::new(path[1], path[2]).unwrap(), (200, 50)),
        ];
        let pools = hashmap! {
            pools[0].tokens => pools[0].clone(),
            pools[1].tokens => pools[1].clone(),
        };

        assert_eq!(estimate_sell_amount(100.into(), &path, &pools), None);
    }

    #[test]
    fn test_estimate_spot_price() {
        let sell_token = H160::from_low_u64_be(1);
        let intermediate = H160::from_low_u64_be(2);
        let buy_token = H160::from_low_u64_be(3);

        let path = vec![sell_token, intermediate, buy_token];
        let pools = [
            Pool::uniswap(TokenPair::new(path[0], path[1]).unwrap(), (100, 100)),
            Pool::uniswap(TokenPair::new(path[1], path[2]).unwrap(), (200, 50)),
        ];
        let pools = hashmap! {
            pools[0].tokens => pools[0].clone(),
            pools[1].tokens => pools[1].clone(),
        };

        assert_eq!(
            big_rational_to_float(&estimate_spot_price(&path, &pools).unwrap()),
            Some(0.25)
        );
    }
}
