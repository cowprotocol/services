//! BalancerV2 specific domain types.

use ethereum_types::U256;

/// A scaling factor used for normalizing token amounts.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct ScalingFactor(U256);

impl ScalingFactor {
    /// Returns the underlying scaling factor value.
    pub fn get(&self) -> U256 {
        self.0
    }
}

impl Default for ScalingFactor {
    fn default() -> Self {
        Self(U256::one())
    }
}
