use {alloy::primitives::Address, model::TokenPair, std::collections::HashSet};

/// The maximum number of hops to use when trading with AMMs along a path.
const DEFAULT_MAX_HOPS: usize = 2;

type PathCandidate = Vec<Address>;

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
    use {super::*, maplit::hashset, model::TokenPair};

    #[test]
    fn path_candidates_empty_when_same_token() {
        let base = BaseTokens::new(Address::with_last_byte(0), &[Address::with_last_byte(1)]);
        let sell_token = Address::with_last_byte(2);
        let buy_token = Address::with_last_byte(2);

        assert!(base.path_candidates(sell_token, buy_token).is_empty());
    }

    #[test]
    fn test_path_candidates() {
        let base_tokens = [
            Address::with_last_byte(0),
            Address::with_last_byte(1),
            Address::with_last_byte(2),
        ];
        let base_token_set: HashSet<Address> = base_tokens.iter().copied().collect();

        let sell_token = Address::with_last_byte(4);
        let buy_token = Address::with_last_byte(5);

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
    fn base_token_pairs_() {
        let base_tokens: Vec<Address> = [0, 1, 2]
            .iter()
            .copied()
            .map(Address::with_last_byte)
            .collect();
        let pairs: Vec<TokenPair> = base_token_pairs(&base_tokens).collect();
        assert_eq!(pairs.len(), 3);
        assert!(pairs.contains(&TokenPair::new(base_tokens[0], base_tokens[1]).unwrap()));
        assert!(pairs.contains(&TokenPair::new(base_tokens[0], base_tokens[2]).unwrap()));
        assert!(pairs.contains(&TokenPair::new(base_tokens[1], base_tokens[2]).unwrap()));
    }

    #[test]
    fn relevant_pairs() {
        let tokens: Vec<Address> = [0, 1, 2, 3, 4]
            .iter()
            .copied()
            .map(Address::with_last_byte)
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
