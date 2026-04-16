use {
    super::{Address, TokenAddress},
    alloy_primitives::{U256, address},
    derive_more::{From, Into},
};

/// An ERC20 allowance.
///
/// https://eips.ethereum.org/EIPS/eip-20
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Allowance {
    /// The token for the allowance.
    pub token: TokenAddress,
    /// The spender address.
    pub spender: Address,
    /// The amount for the allowance.
    pub amount: U256,
}

/// An allowance that's already in effect, this essentially models the result of
/// the allowance() method, see https://eips.ethereum.org/EIPS/eip-20#methods.
#[derive(Debug, Clone, Copy, Into, From)]
pub struct Existing(pub Allowance);

/// An allowance that is required for some action.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Into, From)]
pub struct Required(pub Allowance);

impl Required {
    /// Check if this allowance needs to be approved, and if so, return the
    /// appropriate [`Approval`].
    pub fn approval(&self, existing: &Existing) -> Option<Approval> {
        if self.0.spender != existing.0.spender || self.0.amount <= existing.0.amount {
            None
        } else {
            Some(Approval(self.0))
        }
    }
}

/// An approval which needs to be made with an approve() call, see
/// https://eips.ethereum.org/EIPS/eip-20#methods.
#[derive(Debug, Clone, Copy)]
pub struct Approval(pub Allowance);

impl Approval {
    /// Approve the maximal amount possible, i.e. set the approved amount to
    /// [`U256::max_value`].
    pub fn max(self) -> Self {
        Self(Allowance {
            amount: U256::MAX,
            ..self.0
        })
    }

    /// Revoke the approval, i.e. set the approved amount to [`U256::zero`].
    pub fn revoke(self) -> Self {
        Self(Allowance {
            amount: U256::ZERO,
            ..self.0
        })
    }

    /// Some tokens (e.g. USDT on Mainnet) revert when approving a non-zero
    /// value if the current allowance is also non-zero. For these tokens
    /// the allowance must be reset to 0 before setting a new value.
    pub fn requires_reset(&self) -> bool {
        // Mainnet USDT
        const USDT_MAINNET: Address = address!("dAC17F958D2ee523a2206206994597C13D831ec7");

        self.0.token.0 == USDT_MAINNET
    }
}
