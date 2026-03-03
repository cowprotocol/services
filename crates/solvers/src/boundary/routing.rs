//! Module containing basic path-finding logic to get quotes/routes for the best
//! onchain liquidity.

use {
    alloy::primitives::{Address, U256},
    liquidity_sources::baseline_solvable::BaselineSolvable,
    model::TokenPair,
    std::collections::{HashMap, HashSet},
};

/// The maximum number of hops to use when trading with AMMs along a path.
const DEFAULT_MAX_HOPS: usize = 2;

type PathCandidate = Vec<Address>;

pub struct Estimate<'a, V, L> {
    // The result amount of the estimate
    pub value: V,
    // The liquidity path used to derive at that estimate
    pub path: Vec<&'a L>,
}

impl<V, L: BaselineSolvable> Estimate<'_, V, L> {
    pub async fn gas_cost(&self) -> usize {
        // This could be more accurate by actually simulating the settlement (since
        // different tokens might have more or less expensive transfer costs)
        // For the standard OZ token the cost is roughly 110k for a direct trade, 170k
        // for a 1 hop trade, 230k for a 2 hop trade.
        let costs = self.path.iter().map(|p| p.gas_cost());
        let cost_of_hops: usize = futures::future::join_all(costs).await.into_iter().sum();
        50_000 + cost_of_hops
    }
}

// Given a path and sell amount (first token of the path) estimates the buy
// amount (last token of the path) and the path of liquidity that yields this
// result Returns None if the path is invalid or pool information doesn't exist.
pub async fn estimate_buy_amount<'a, L: BaselineSolvable>(
    sell_amount: U256,
    path: &[Address],
    liquidity: &'a HashMap<TokenPair, Vec<L>>,
) -> Option<Estimate<'a, U256, L>> {
    let sell_token = path.first()?;

    let mut previous = (sell_amount, *sell_token, Vec::new());

    for current in path.iter().skip(1) {
        let (amount, previous_token, mut path) = previous;
        let pools = liquidity.get(&TokenPair::new(*current, previous_token)?)?;
        let outputs = futures::future::join_all(pools.iter().map(|liquidity| async move {
            let output = liquidity
                .get_amount_out(*current, (amount, previous_token))
                .await;
            output.map(|output| (liquidity, output))
        }))
        .await;
        let (best_liquidity, amount) = outputs
            .into_iter()
            .flatten()
            .max_by_key(|(_, amount)| *amount)?;
        path.push(best_liquidity);
        previous = (amount, *current, path);
    }

    let (buy_amount, _, path) = previous;
    Some(Estimate {
        value: buy_amount,
        path,
    })
}

// Given a path and buy amount (last token of the path) estimates the sell
// amount (first token of the path) and the path of liquidity that yields this
// result Returns None if the path is invalid or pool information doesn't exist.
pub async fn estimate_sell_amount<'a, L: BaselineSolvable>(
    buy_amount: U256,
    path: &[Address],
    liquidity: &'a HashMap<TokenPair, Vec<L>>,
) -> Option<Estimate<'a, U256, L>> {
    let buy_token = path.last()?;

    let mut previous = (buy_amount, *buy_token, Vec::new());

    for current in path.iter().rev().skip(1) {
        let (amount, previous_token, mut path) = previous;
        let pools = liquidity.get(&TokenPair::new(*current, previous_token)?)?;
        let outputs = futures::future::join_all(pools.iter().map(|liquidity| async move {
            let output = liquidity
                .get_amount_in(*current, (amount, previous_token))
                .await;
            output.map(|output| (liquidity, output))
        }))
        .await;
        let (best_liquidity, amount) = outputs
            .into_iter()
            .flatten()
            .min_by_key(|(_, amount)| *amount)?;
        path.push(best_liquidity);
        previous = (amount, *current, path);
    }

    let (sell_amount, _, mut path) = previous;
    // Since we reversed the path originally, we need to re-reverse it here.
    path.reverse();

    Some(Estimate {
        value: sell_amount,
        path,
    })
}

pub struct BaseTokens {
    /// The base tokens used to determine potential paths in the baseline
    /// solver.
    ///
    /// Always includes the native token.
    tokens: HashSet<Address>,
    /// All pairs of above.
    pairs: HashSet<TokenPair>,
}

impl BaseTokens {
    pub fn new(native_token: Address, base_tokens: &[Address]) -> Self {
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

    pub fn tokens(&self) -> &HashSet<Address> {
        &self.tokens
    }

    /// All pool token pairs that could be used along a path candidate for these
    /// token pairs.
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
        // Could be empty if the input pairs are empty. Just like path_candidates we
        // return empty set in this case.
        if !result.is_empty() {
            result.extend(self.pairs.iter().copied());
        }
        result
    }

    // Returns possible paths from sell_token to buy token, given a list of
    // potential intermediate base tokens and a maximum number of intermediate
    // steps. Can contain token pairs between base tokens or a base token and
    // the sell or buy token.
    pub fn path_candidates(
        &self,
        sell_token: Address,
        buy_token: Address,
    ) -> HashSet<PathCandidate> {
        self.path_candidates_with_hops(sell_token, buy_token, DEFAULT_MAX_HOPS)
    }

    /// Returns possible path candidates with the specified number of maximum
    /// hops.
    pub fn path_candidates_with_hops(
        &self,
        sell_token: Address,
        buy_token: Address,
        max_hops: usize,
    ) -> HashSet<PathCandidate> {
        path_candidates(sell_token, buy_token, &self.tokens, max_hops)
    }
}

fn path_candidates(
    sell_token: Address,
    buy_token: Address,
    base_tokens: &HashSet<Address>,
    max_hops: usize,
) -> HashSet<PathCandidate> {
    if sell_token == buy_token {
        return HashSet::new();
    }

    let mut candidates = HashSet::new();

    // Start with just the sell token (yields the direct pair candidate in the 0th
    // iteration)
    let mut path_prefixes = vec![vec![sell_token]];
    for _ in 0..(max_hops + 1) {
        let mut next_round_path_prefixes = vec![];
        for path_prefix in &path_prefixes {
            // For this round, add the buy token and path to the candidates
            let mut full_path = path_prefix.clone();
            full_path.push(buy_token);
            candidates.insert(full_path);

            // For the next round, amend current prefix with all base tokens that are not
            // yet on the path
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
fn base_token_pairs(base_tokens: &[Address]) -> impl Iterator<Item = TokenPair> + '_ {
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
    use {
        super::*,
        liquidity_sources::uniswap_v2::pool_fetching::Pool,
        maplit::hashmap,
        model::TokenPair,
    };

    #[tokio::test]
    async fn test_estimate_amount_returns_none_if_it_contains_pair_without_pool() {
        let sell_token = Address::with_last_byte(1);
        let intermediate = Address::with_last_byte(2);
        let buy_token = Address::with_last_byte(3);

        let path = vec![sell_token, intermediate, buy_token];
        let pools = [Pool::uniswap(
            Address::with_last_byte(1),
            TokenPair::new(path[0], path[1]).unwrap(),
            (100, 100),
        )];
        let pools = hashmap! {
            pools[0].tokens => vec![pools[0]],
        };

        assert!(
            estimate_buy_amount(U256::ONE, &path, &pools)
                .await
                .is_none()
        );
        assert!(
            estimate_sell_amount(U256::ONE, &path, &pools)
                .await
                .is_none()
        );
    }

    #[tokio::test]
    async fn test_estimate_amount() {
        let sell_token = Address::with_last_byte(1);
        let intermediate = Address::with_last_byte(2);
        let buy_token = Address::with_last_byte(3);

        let path = vec![sell_token, intermediate, buy_token];
        let pools = [
            Pool::uniswap(
                Address::with_last_byte(1),
                TokenPair::new(path[0], path[1]).unwrap(),
                (100, 100),
            ),
            Pool::uniswap(
                Address::with_last_byte(2),
                TokenPair::new(path[1], path[2]).unwrap(),
                (200, 50),
            ),
        ];
        let pools = hashmap! {
            pools[0].tokens => vec![pools[0]],
            pools[1].tokens => vec![pools[1]],
        };

        assert_eq!(
            estimate_buy_amount(U256::from(10), &path, &pools)
                .await
                .unwrap()
                .value,
            U256::from(2)
        );

        assert_eq!(
            estimate_sell_amount(U256::from(10), &path, &pools)
                .await
                .unwrap()
                .value,
            U256::from(105)
        );
    }

    #[tokio::test]
    async fn test_estimate_sell_amount_returns_none_buying_too_much() {
        let sell_token = Address::with_last_byte(1);
        let intermediate = Address::with_last_byte(2);
        let buy_token = Address::with_last_byte(3);

        let path = vec![sell_token, intermediate, buy_token];
        let pools = [
            Pool::uniswap(
                Address::with_last_byte(1),
                TokenPair::new(path[0], path[1]).unwrap(),
                (100, 100),
            ),
            Pool::uniswap(
                Address::with_last_byte(2),
                TokenPair::new(path[1], path[2]).unwrap(),
                (200, 50),
            ),
        ];
        let pools = hashmap! {
            pools[0].tokens => vec![pools[0]],
            pools[1].tokens => vec![pools[1]],
        };

        assert!(
            estimate_sell_amount(U256::from(100), &path, &pools)
                .await
                .is_none()
        );
    }

    #[tokio::test]
    async fn test_estimate_amount_multiple_pools() {
        let sell_token = Address::with_last_byte(1);
        let intermediate = Address::with_last_byte(2);
        let buy_token = Address::with_last_byte(3);

        let mut path = vec![sell_token, intermediate, buy_token];
        let first_pair = TokenPair::new(path[0], path[1]).unwrap();
        let second_pair = TokenPair::new(path[1], path[2]).unwrap();

        let first_hop_high_price =
            Pool::uniswap(Address::with_last_byte(1), first_pair, (101_000, 100_000));
        let first_hop_low_price =
            Pool::uniswap(Address::with_last_byte(1), first_pair, (100_000, 101_000));
        let second_hop_high_slippage =
            Pool::uniswap(Address::with_last_byte(2), second_pair, (200_000, 50_000));
        let second_hop_low_slippage = Pool::uniswap(
            Address::with_last_byte(2),
            second_pair,
            (200_000_000, 50_000_000),
        );
        let pools = hashmap! {
            first_pair => vec![first_hop_high_price, first_hop_low_price],
            second_pair => vec![second_hop_high_slippage, second_hop_low_slippage],
        };

        let buy_estimate = estimate_buy_amount(U256::from(1000), &path, &pools)
            .await
            .unwrap();
        assert_eq!(
            buy_estimate.path,
            [&first_hop_low_price, &second_hop_low_slippage]
        );

        let sell_estimate = estimate_sell_amount(U256::from(1000), &path, &pools)
            .await
            .unwrap();
        assert_eq!(
            sell_estimate.path,
            [&first_hop_low_price, &second_hop_low_slippage]
        );

        // For the reverse path we now expect to use the higher price for the first hop,
        // but still low slippage for the second
        path.reverse();
        let buy_estimate = estimate_buy_amount(U256::from(1000), &path, &pools)
            .await
            .unwrap();
        assert_eq!(
            buy_estimate.path,
            [&second_hop_low_slippage, &first_hop_high_price]
        );

        let sell_estimate = estimate_sell_amount(U256::from(1000), &path, &pools)
            .await
            .unwrap();
        assert_eq!(
            sell_estimate.path,
            [&second_hop_low_slippage, &first_hop_high_price]
        );
    }

    #[tokio::test]
    async fn test_estimate_amount_invalid_pool() {
        let sell_token = Address::with_last_byte(1);
        let buy_token = Address::with_last_byte(2);
        let pair = TokenPair::new(sell_token, buy_token).unwrap();

        let path = vec![sell_token, buy_token];
        let valid_pool = Pool::uniswap(Address::with_last_byte(1), pair, (100_000, 100_000));
        let invalid_pool = Pool::uniswap(Default::default(), pair, (0, 0));
        let pools = hashmap! {
            pair => vec![valid_pool, invalid_pool],
        };

        let buy_estimate = estimate_buy_amount(U256::from(1000), &path, &pools)
            .await
            .unwrap();
        assert_eq!(buy_estimate.path, [&valid_pool]);

        let sell_estimate = estimate_sell_amount(U256::from(1000), &path, &pools)
            .await
            .unwrap();
        assert_eq!(sell_estimate.path, [&valid_pool]);
    }
}
