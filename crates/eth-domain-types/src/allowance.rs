use {
    super::{Address, TokenAddress},
    alloy_primitives::{U256, address},
    chain::Chain,
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

    /// Some tokens (e.g. USDT) revert when approving a non-zero value if the
    /// current allowance is also non-zero. For these tokens the allowance must
    /// be reset to 0 before setting a new value.
    pub fn requires_reset(&self, chain: Chain) -> bool {
        let tokens: &[Address] = match chain {
            Chain::Mainnet => &[address!("dAC17F958D2ee523a2206206994597C13D831ec7")],
            Chain::ArbitrumOne => &[address!("Fd086bC7CD5C481DCC9C85ebE478A1C0b69FCbb9")],
            Chain::Gnosis => &[address!("4ECaBa5870353805a9F068101A40E0f32ed605C6")],
            Chain::Optimism => &[address!("94b008aA00579c1307B0EF2c499aD98a8ce58e58")],
            Chain::Polygon => &[address!("c2132D05D31c914a87C6611C10748AEb04B58e8F")],
            Chain::Avalanche => &[address!("9702230A8Ea53601f5cD2dc00fDBc13d4dF4A8c7")],
            Chain::Bnb => &[address!("55d398326f99059fF775485246999027B3197955")],
            Chain::Base => &[address!("fde4C96c8593536E31F229EA8f37b2ADa2699bb2")],
            _ => &[],
        };

        tokens.contains(&self.0.token.0)
    }
}
