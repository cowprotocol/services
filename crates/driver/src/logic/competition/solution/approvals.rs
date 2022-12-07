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
