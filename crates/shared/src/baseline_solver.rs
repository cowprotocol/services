//! Module containing basic path-finding logic to get quotes/routes for the best onchain liquidity.

use ethcontract::{H160, U256};
use model::TokenPair;
use std::collections::{HashMap, HashSet};

/// The maximum number of hops to use when trading with AMMs along a path.
const DEFAULT_MAX_HOPS: usize = 2;

type PathCandidate = Vec<H160>;

/// Note that get_amount_out and get_amount_in are not always symmetrical. That is for some AMMs it
/// is possible that get_amount_out returns an amount for which get_amount_in returns None when
/// trying to go the reverse direction. Or that the resulting amount is different from the original.
/// This situation is rare and resulting amounts should usually be identical or very close but it
/// can occur.
pub trait BaselineSolvable {
    // Given the desired output token, the amount and token input, return the expected amount of output token.
    fn get_amount_out(&self, out_token: H160, input: (U256, H160)) -> Option<U256>;

    // Given the input token, the amount and token we want output, return the required amount of input token that needs to be provided.
    fn get_amount_in(&self, in_token: H160, out: (U256, H160)) -> Option<U256>;

    // Returns the approximate amount of gas that using this piece of liquidity would incur
    fn gas_cost(&self) -> usize;
}

pub struct Estimate<'a, V, L> {
    // The result amount of the estimate
    pub value: V,
    // The liquidity path used to derive at that estimate
    pub path: Vec<&'a L>,
}

impl<'a, V, L: BaselineSolvable> Estimate<'a, V, L> {
    pub fn gas_cost(&self) -> usize {
        // This could be more accurate by actually simulating the settlement (since different tokens might have more or less expensive transfer costs)
        // For the standard OZ token the cost is roughly 110k for a direct trade, 170k for a 1 hop trade, 230k for a 2 hop trade.
        let cost_of_hops: usize = self.path.iter().map(|item| item.gas_cost()).sum();
        50_000 + cost_of_hops
    }
}

// Given a path and sell amount (first token of the path) estimates the buy amount (last token of the path) and
// the path of liquidity that yields this result
// Returns None if the path is invalid or pool information doesn't exist.
pub fn estimate_buy_amount<'a, L: BaselineSolvable>(
    sell_amount: U256,
    path: &[H160],
    liquidity: &'a HashMap<TokenPair, Vec<L>>,
) -> Option<Estimate<'a, U256, L>> {
    let sell_token = path.first()?;
    path.iter()
        .skip(1)
        .fold(
            Some((sell_amount, *sell_token, Vec::new())),
            |previous, current| {
                let (amount, previous, mut path) = previous?;
                let (best_liquidity, amount) = liquidity
                    .get(&TokenPair::new(*current, previous)?)?
                    .iter()
                    .map(|liquidity| {
                        (
                            liquidity,
                            liquidity.get_amount_out(*current, (amount, previous)),
                        )
                    })
                    .max_by(|(_, amount_a), (_, amount_b)| amount_a.cmp(amount_b))?;
                path.push(best_liquidity);
                Some((amount?, *current, path))
            },
        )
        .map(|(amount, _, liquidity)| Estimate {
            value: amount,
            path: liquidity,
        })
}

// Given a path and buy amount (last token of the path) estimates the sell amount (first token of the path) and
// the path of liquidity that yields this result
// Returns None if the path is invalid or pool information doesn't exist.
pub fn estimate_sell_amount<'a, L: BaselineSolvable>(
    buy_amount: U256,
    path: &[H160],
    liquidity: &'a HashMap<TokenPair, Vec<L>>,
) -> Option<Estimate<'a, U256, L>> {
    let buy_token = path.last()?;
    path.iter()
        .rev()
        .skip(1)
        .fold(
            Some((buy_amount, *buy_token, Vec::new())),
            |previous, current| {
                let (amount, previous, mut path) = previous?;
                let (best_liquidity, amount) = liquidity
                    .get(&TokenPair::new(*current, previous)?)?
                    .iter()
                    .map(|liquidity| {
                        (
                            liquidity,
                            liquidity.get_amount_in(*current, (amount, previous)),
                        )
                    })
                    .min_by(|(_, amount_a), (_, amount_b)| {
                        amount_a
                            .unwrap_or_else(U256::max_value)
                            .cmp(&amount_b.unwrap_or_else(U256::max_value))
                    })?;
                path.push(best_liquidity);
                Some((amount?, *current, path))
            },
        )
        .map(|(amount, _, liquidity)| Estimate {
            value: amount,
            // Since we reversed the path originally, we need to re-reverse it here.
            path: liquidity.into_iter().rev().collect(),
        })
}

pub struct BaseTokens {
    /// The base tokens used to determine potential paths in the baseline solver.
    ///
    /// Always includes the native token.
    tokens: HashSet<H160>,
    /// All pairs of above.
    pairs: HashSet<TokenPair>,
}

impl BaseTokens {
    pub fn new(native_token: H160, base_tokens: &[H160]) -> Self {
        let mut tokens = base_tokens.to_vec();
        tokens.push(native_token);
        tokens.sort();
        tokens.dedup();
        let pairs = base_token_pairs(&tokens).collect();
        Self {
            tokens: tokens.into_iter().collect(),
            pairs,
        }
    }

    pub fn tokens(&self) -> &HashSet<H160> {
        &self.tokens
    }

    /// All pool token pairs that could be used along a path candidate for these token pairs.
    pub fn relevant_pairs(&self, pairs: impl Iterator<Item = TokenPair>) -> HashSet<TokenPair> {
        let mut result = HashSet::new();
        for pair in pairs {
            result.insert(pair);
            for token in pair {
                result.extend(
                    self.tokens
                        .iter()
                        .filter_map(move |base_token| TokenPair::new(*base_token, token)),
                );
            }
        }
        // Could be empty if the input pairs are empty. Just like path_candidates we return empty
        // set in this case.
        if !result.is_empty() {
            result.extend(self.pairs.iter().copied());
        }
        result
    }

    // Returns possible paths from sell_token to buy token, given a list of potential intermediate base tokens
    // and a maximum number of intermediate steps.
    // Can contain token pairs between base tokens or a base token and the sell or buy token.
    pub fn path_candidates(&self, sell_token: H160, buy_token: H160) -> HashSet<PathCandidate> {
        path_candidates(sell_token, buy_token, &self.tokens, DEFAULT_MAX_HOPS)
    }
}

fn path_candidates(
    sell_token: H160,
    buy_token: H160,
    base_tokens: &HashSet<H160>,
    max_hops: usize,
) -> HashSet<PathCandidate> {
    if sell_token == buy_token {
        return HashSet::new();
    }

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

/// All token pairs between base tokens.
fn base_token_pairs(base_tokens: &[H160]) -> impl Iterator<Item = TokenPair> + '_ {
    base_tokens
        .iter()
        .copied()
        .enumerate()
        .flat_map(move |(index, token)| {
            base_tokens
                .iter()
                .copied()
                .skip(index)
                .filter_map(move |token_| TokenPair::new(token, token_))
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sources::uniswap_v2::pool_fetching::Pool;
    use ethcontract::H160;
    use maplit::{hashmap, hashset};
    use model::TokenPair;

    #[test]
    fn path_candidates_empty_when_same_token() {
        let base = BaseTokens::new(H160::from_low_u64_be(0), &[H160::from_low_u64_be(1)]);
        let sell_token = H160::from_low_u64_be(2);
        let buy_token = H160::from_low_u64_be(2);

        assert!(base.path_candidates(sell_token, buy_token).is_empty());
    }

    #[test]
    fn test_path_candidates() {
        let base_tokens = vec![
            H160::from_low_u64_be(0),
            H160::from_low_u64_be(1),
            H160::from_low_u64_be(2),
        ];
        let base_token_set: HashSet<H160> = base_tokens.iter().copied().collect();

        let sell_token = H160::from_low_u64_be(4);
        let buy_token = H160::from_low_u64_be(5);

        // 0 hops
        assert_eq!(
            path_candidates(sell_token, buy_token, &base_token_set, 0),
            hashset! {vec![sell_token, buy_token]}
        );

        // 1 hop with all permutations
        assert_eq!(
            path_candidates(sell_token, buy_token, &base_token_set, 1),
            hashset! {
                vec![sell_token, buy_token],
                vec![sell_token, base_tokens[0], buy_token],
                vec![sell_token, base_tokens[1], buy_token],
                vec![sell_token, base_tokens[2], buy_token],

            }
        );

        // 2 & 3 hops check count
        assert_eq!(
            path_candidates(sell_token, buy_token, &base_token_set, 2).len(),
            10
        );
        assert_eq!(
            path_candidates(sell_token, buy_token, &base_token_set, 3).len(),
            16
        );

        // 4 hops should not yield any more permutations since we used all base tokens
        assert_eq!(
            path_candidates(sell_token, buy_token, &base_token_set, 4).len(),
            16
        );

        // Ignores base token if part of buy or sell
        assert_eq!(
            path_candidates(base_tokens[0], buy_token, &base_token_set, 1),
            hashset! {
                vec![base_tokens[0], buy_token],
                vec![base_tokens[0], base_tokens[1], buy_token],
                vec![base_tokens[0], base_tokens[2], buy_token],

            }
        );
        assert_eq!(
            path_candidates(sell_token, base_tokens[0], &base_token_set, 1),
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
            H160::from_low_u64_be(1),
            TokenPair::new(path[0], path[1]).unwrap(),
            (100, 100),
        )];
        let pools = hashmap! {
            pools[0].tokens => vec![pools[0]],
        };

        assert!(estimate_buy_amount(1.into(), &path, &pools).is_none());
        assert!(estimate_sell_amount(1.into(), &path, &pools).is_none());
    }

    #[test]
    fn test_estimate_amount() {
        let sell_token = H160::from_low_u64_be(1);
        let intermediate = H160::from_low_u64_be(2);
        let buy_token = H160::from_low_u64_be(3);

        let path = vec![sell_token, intermediate, buy_token];
        let pools = [
            Pool::uniswap(H160::from_low_u64_be(1), TokenPair::new(path[0], path[1]).unwrap(), (100, 100)),
            Pool::uniswap(H160::from_low_u64_be(2), TokenPair::new(path[1], path[2]).unwrap(), (200, 50)),
        ];
        let pools = hashmap! {
            pools[0].tokens => vec![pools[0]],
            pools[1].tokens => vec![pools[1]],
        };

        assert_eq!(
            estimate_buy_amount(10.into(), &path, &pools).unwrap().value,
            2.into()
        );

        assert_eq!(
            estimate_sell_amount(10.into(), &path, &pools)
                .unwrap()
                .value,
            105.into()
        );
    }

    #[test]
    fn test_estimate_sell_amount_returns_none_buying_too_much() {
        let sell_token = H160::from_low_u64_be(1);
        let intermediate = H160::from_low_u64_be(2);
        let buy_token = H160::from_low_u64_be(3);

        let path = vec![sell_token, intermediate, buy_token];
        let pools = [
            Pool::uniswap(H160::from_low_u64_be(1), TokenPair::new(path[0], path[1]).unwrap(), (100, 100)),
            Pool::uniswap(H160::from_low_u64_be(2), TokenPair::new(path[1], path[2]).unwrap(), (200, 50)),
        ];
        let pools = hashmap! {
            pools[0].tokens => vec![pools[0]],
            pools[1].tokens => vec![pools[1]],
        };

        assert!(estimate_sell_amount(100.into(), &path, &pools).is_none());
    }

    #[test]
    fn test_estimate_amount_multiple_pools() {
        let sell_token = H160::from_low_u64_be(1);
        let intermediate = H160::from_low_u64_be(2);
        let buy_token = H160::from_low_u64_be(3);

        let mut path = vec![sell_token, intermediate, buy_token];
        let first_pair = TokenPair::new(path[0], path[1]).unwrap();
        let second_pair = TokenPair::new(path[1], path[2]).unwrap();

        let first_hop_high_price = Pool::uniswap(H160::from_low_u64_be(1), first_pair, (101_000, 100_000));
        let first_hop_low_price = Pool::uniswap(H160::from_low_u64_be(1), first_pair, (100_000, 101_000));
        let second_hop_high_slippage = Pool::uniswap(H160::from_low_u64_be(2), second_pair, (200_000, 50_000));
        let second_hop_low_slippage = Pool::uniswap(H160::from_low_u64_be(2), second_pair, (200_000_000, 50_000_000));
        let pools = hashmap! {
            first_pair => vec![first_hop_high_price, first_hop_low_price],
            second_pair => vec![second_hop_high_slippage, second_hop_low_slippage],
        };

        let buy_estimate = estimate_buy_amount(1000.into(), &path, &pools).unwrap();
        assert_eq!(
            buy_estimate.path,
            [&first_hop_low_price, &second_hop_low_slippage]
        );

        let sell_estimate = estimate_sell_amount(1000.into(), &path, &pools).unwrap();
        assert_eq!(
            sell_estimate.path,
            [&first_hop_low_price, &second_hop_low_slippage]
        );

        // For the reverse path we now expect to use the higher price for the first hop, but still low slippage for the second
        path.reverse();
        let buy_estimate = estimate_buy_amount(1000.into(), &path, &pools).unwrap();
        assert_eq!(
            buy_estimate.path,
            [&second_hop_low_slippage, &first_hop_high_price]
        );

        let sell_estimate = estimate_sell_amount(1000.into(), &path, &pools).unwrap();
        assert_eq!(
            sell_estimate.path,
            [&second_hop_low_slippage, &first_hop_high_price]
        );
    }

    #[test]
    fn test_estimate_amount_invalid_pool() {
        let sell_token = H160::from_low_u64_be(1);
        let buy_token = H160::from_low_u64_be(2);
        let pair = TokenPair::new(sell_token, buy_token).unwrap();

        let path = vec![sell_token, buy_token];
        let valid_pool = Pool::uniswap(H160::from_low_u64_be(1), pair, (100_000, 100_000));
        let invalid_pool = Pool::uniswap(H160::default(), pair, (0, 0));
        let pools = hashmap! {
            pair => vec![valid_pool, invalid_pool],
        };

        let buy_estimate = estimate_buy_amount(1000.into(), &path, &pools).unwrap();
        assert_eq!(buy_estimate.path, [&valid_pool]);

        let sell_estimate = estimate_sell_amount(1000.into(), &path, &pools).unwrap();
        assert_eq!(sell_estimate.path, [&valid_pool]);
    }

    #[test]
    fn base_token_pairs_() {
        let base_tokens: Vec<H160> = [0, 1, 2]
            .iter()
            .copied()
            .map(H160::from_low_u64_le)
            .collect();
        let pairs: Vec<TokenPair> = base_token_pairs(&base_tokens).collect();
        assert_eq!(pairs.len(), 3);
        assert!(pairs.contains(&TokenPair::new(base_tokens[0], base_tokens[1]).unwrap()));
        assert!(pairs.contains(&TokenPair::new(base_tokens[0], base_tokens[2]).unwrap()));
        assert!(pairs.contains(&TokenPair::new(base_tokens[1], base_tokens[2]).unwrap()));
    }

    #[test]
    fn relevant_pairs() {
        let tokens: Vec<H160> = [0, 1, 2, 3, 4]
            .iter()
            .copied()
            .map(H160::from_low_u64_le)
            .collect();
        let base = BaseTokens::new(tokens[0], &tokens[1..2]);

        let pairs = base.relevant_pairs(&mut std::iter::empty());
        assert!(pairs.is_empty());

        let pairs = base.relevant_pairs(&mut TokenPair::new(tokens[0], tokens[1]).into_iter());
        assert_eq!(pairs.len(), 1);
        assert!(pairs.contains(&TokenPair::new(tokens[0], tokens[1]).unwrap()));

        let pairs = base.relevant_pairs(&mut TokenPair::new(tokens[3], tokens[4]).into_iter());
        assert_eq!(pairs.len(), 6);
        assert!(pairs.contains(&TokenPair::new(tokens[0], tokens[1]).unwrap()));
        assert!(pairs.contains(&TokenPair::new(tokens[0], tokens[3]).unwrap()));
        assert!(pairs.contains(&TokenPair::new(tokens[0], tokens[4]).unwrap()));
        assert!(pairs.contains(&TokenPair::new(tokens[1], tokens[3]).unwrap()));
        assert!(pairs.contains(&TokenPair::new(tokens[1], tokens[4]).unwrap()));
        assert!(pairs.contains(&TokenPair::new(tokens[3], tokens[4]).unwrap()));
    }
}
