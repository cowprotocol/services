//! The benchmark's working set: the `(owner, sell_token)` pairs the autopilot
//! reads balances for.

use {
    account_balances::Query,
    alloy_primitives::Address,
    anyhow::{Context, Result},
    model::order::SellTokenSource,
    serde::{Deserialize, Serialize},
    std::{collections::HashSet, path::Path},
};

#[derive(Debug, Serialize, Deserialize)]
pub struct Fixture {
    /// Which network the pairs were taken from. Replaying them against a
    /// different chain measures nothing, so `run` refuses to mismatch.
    pub network: String,
    /// When the dump was taken, as an ISO timestamp. Informational only.
    pub dumped_at: String,
    pub pairs: Vec<Pair>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct Pair {
    pub owner: Address,
    pub token: Address,
}

impl Fixture {
    pub fn load(path: &Path) -> Result<Self> {
        let bytes = std::fs::read(path)
            .with_context(|| format!("could not read fixture {}", path.display()))?;
        serde_json::from_slice(&bytes).context("could not parse fixture")
    }

    pub fn store(&self, path: &Path) -> Result<()> {
        let bytes = serde_json::to_vec_pretty(self)?;
        std::fs::write(path, bytes)
            .with_context(|| format!("could not write fixture {}", path.display()))
    }

    /// Only the interaction-free ERC20 path is benchmarked, which is what the
    /// dump filters for, so every pair becomes a plain query.
    pub fn queries(&self) -> Vec<Query> {
        self.pairs
            .iter()
            .map(|pair| Query {
                owner: pair.owner,
                token: pair.token,
                source: SellTokenSource::Erc20,
                interactions: vec![],
                balance_override: None,
            })
            .collect()
    }

    /// Token and owner diversity decide whether the comparison means anything —
    /// a working set concentrated on a few tokens sits entirely in the node's
    /// state cache and makes the unbatched path look artificially good.
    pub fn diversity(&self) -> (usize, usize) {
        let tokens: HashSet<_> = self.pairs.iter().map(|pair| pair.token).collect();
        let owners: HashSet<_> = self.pairs.iter().map(|pair| pair.owner).collect();
        (tokens.len(), owners.len())
    }
}
