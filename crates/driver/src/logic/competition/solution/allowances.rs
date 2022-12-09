use {crate::logic::eth, primitive_types::U256, std::collections::HashMap};

/// A set of ERC20 allowances required by the [`super::Solution`]. This type
/// guarantees that there is only one allowance per [`eth::allowance::Spender`]
/// and that the allowances are sorted deterministically.
#[derive(Debug)]
pub struct Allowances(Vec<eth::allowance::Required>);

impl Allowances {
    /// Normalize the allowances such that there is only one allowance per
    /// [`eth::allowance::Spender`] (by summing them) and order them
    /// deterministically.
    pub fn normalize(allowances: impl Iterator<Item = eth::allowance::Required>) -> Self {
        let mut normalized = HashMap::new();
        for allowance in allowances {
            let amount = normalized
                .entry(allowance.0.spender)
                .or_insert(U256::zero());
            *amount = amount.saturating_add(allowance.0.amount);
        }
        let mut normalized: Vec<_> = normalized
            .into_iter()
            .map(|(spender, amount)| eth::Allowance { spender, amount }.into())
            .collect();
        normalized.sort();
        Self(normalized)
    }

    pub fn spenders(&self) -> impl Iterator<Item = eth::allowance::Spender> + '_ {
        self.0.iter().map(|allowance| allowance.0.spender)
    }
}

impl IntoIterator for Allowances {
    type IntoIter = std::vec::IntoIter<Self::Item>;
    type Item = eth::allowance::Required;

    fn into_iter(self) -> Self::IntoIter {
        self.0.into_iter()
    }
}
