// TODO approval will need to be submodule of solution. It should be implemented
// such that the Approvals are only those approvals which are not already
// covered by allowances (word this better or just read ERC20 and use their
// terminology, also leave the EIP-20 link there) and also Approvals doesn't
// have any duplicates.
//
// TODO I think that all the onchain contract stuff should be defined in the
// node module and there should be specific types there for calling various
// methods. Probably have an enum for it or something. See how it works out.

use {crate::logic::eth, primitive_types::U256, std::collections::HashMap};

/// A set of ERC20 approvals required by the [`super::Solution`]. This type
/// guarantees that there is only one approval per [`eth::Spender`] and that the
/// approvals are sorted deterministically.
#[derive(Debug, Clone)]
pub struct Approvals(Vec<eth::Approval>);

impl Approvals {
    /// Normalize the approvals such that there is only one approval per
    /// [`eth::Spender`] and they are ordered deterministically.
    pub fn normalize(approvals: impl Iterator<Item = eth::Approval>) -> Self {
        let mut normalized = HashMap::new();
        for approval in approvals {
            let amount = normalized.entry(approval.spender).or_insert(U256::zero());
            *amount = amount.checked_add(approval.amount).unwrap();
        }
        let mut normalized: Vec<_> = normalized
            .into_iter()
            .map(|(spender, amount)| eth::Approval { spender, amount })
            .collect();
        normalized.sort();
        Self(normalized)
    }

    pub fn iter(&self) -> impl Iterator<Item = &eth::Approval> {
        self.0.iter()
    }
}
