use {
    super::{Address, Token},
    crate::boundary,
    primitive_types::U256,
};

/// An ERC20 allowance.
///
/// https://eips.ethereum.org/EIPS/eip-20
#[derive(Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Allowance {
    pub spender: Spender,
    pub amount: U256,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Spender {
    pub address: Address,
    pub token: Token,
}

/// An allowance that is required for some action.
#[derive(Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Required(pub Allowance);

/// An allowance that's already in effect, this essentially models the result of
/// the allowance() method, see https://eips.ethereum.org/EIPS/eip-20#methods.
#[derive(Debug)]
pub struct Existing(pub Allowance);

/// An approval which needs to be made with an approve() call, see
/// https://eips.ethereum.org/EIPS/eip-20#methods.
#[derive(Debug)]
pub struct Approval(Allowance);

impl Required {
    /// Check if this allowance needs to be approved, and if so, return the
    /// appropriate [`Approval`].
    pub fn approval(self, existing: &Existing) -> Option<Approval> {
        if self.0.spender != existing.0.spender || self.0.amount <= existing.0.amount {
            None
        } else {
            Some(Approval(self.0))
        }
    }
}

impl From<Allowance> for Required {
    fn from(inner: Allowance) -> Self {
        Self(inner)
    }
}

impl From<Allowance> for Existing {
    fn from(inner: Allowance) -> Self {
        Self(inner)
    }
}

impl From<Approval> for boundary::Approval {
    fn from(approval: Approval) -> Self {
        boundary::Approval {
            token: approval.0.spender.token.0,
            spender: approval.0.spender.address.0,
        }
    }
}
