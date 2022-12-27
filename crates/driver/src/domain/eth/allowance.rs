use super::{Address, TokenAddress};

/// An ERC20 allowance.
///
/// https://eips.ethereum.org/EIPS/eip-20
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Allowance {
    pub spender: Spender,
    pub amount: super::U256,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Spender {
    /// The spender address.
    pub address: Address,
    /// The token being spent.
    pub token: TokenAddress,
}

/// An allowance that's already in effect, this essentially models the result of
/// the allowance() method, see https://eips.ethereum.org/EIPS/eip-20#methods.
#[derive(Debug, Clone, Copy)]
pub struct Existing(pub Allowance);

impl From<Allowance> for Existing {
    fn from(inner: Allowance) -> Self {
        Self(inner)
    }
}

/// An allowance that is required for some action.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Required(pub Allowance);

impl From<Allowance> for Required {
    fn from(inner: Allowance) -> Self {
        Self(inner)
    }
}

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
            amount: super::U256::max_value(),
            ..self.0
        })
    }
}
