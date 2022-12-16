use {
    crate::{blockchain, logic::eth, Ethereum},
    futures::future::try_join_all,
    itertools::Itertools,
    primitive_types::U256,
    std::collections::HashMap,
};

/// A set of ERC20 approvals required by the [`super::Solution`]. This type
/// guarantees that there is only one approval per [`eth::allowance::Spender`]
/// and that the approvals are sorted deterministically, which is important for
/// settlement encoding.
#[derive(Debug, Default)]
pub struct Approvals(Vec<eth::allowance::Approval>);

impl Approvals {
    /// Generate needed approvals for the given allowances.
    pub async fn new(
        eth: &Ethereum,
        allowances: impl Iterator<Item = eth::allowance::Required>,
    ) -> Result<Self, blockchain::Error> {
        let mut normalized = HashMap::new();
        for allowance in allowances {
            let amount = normalized
                .entry(allowance.0.spender)
                .or_insert(U256::zero());
            *amount = amount.saturating_add(allowance.0.amount);
        }
        let allowances = normalized
            .into_iter()
            .map(|(spender, amount)| eth::Allowance { spender, amount }.into())
            .sorted();
        Self::approvals(eth, allowances)
            .await
            .map(Iterator::collect)
            .map(Self)
    }

    async fn approvals(
        eth: &Ethereum,
        allowances: impl Iterator<Item = eth::allowance::Required>,
    ) -> Result<impl Iterator<Item = eth::allowance::Approval>, blockchain::Error> {
        let settlement_contract = eth.contracts().settlement().await?;
        let allowances = try_join_all(allowances.map(|required| async {
            eth.allowance(settlement_contract.address().into(), required.0.spender)
                .await
                .map(|existing| (required, existing))
        }))
        .await?;
        let approvals = allowances.into_iter().filter_map(|(required, existing)| {
            required
                .approval(&existing)
                // As a gas optimization, we always approve the max amount possible. This minimizes
                // the number of approvals necessary, and therefore minimizes the approval fees over time. This is a
                // potential security issue, but its effects are minimized and only exploitable if
                // solvers use insecure contracts.
                .map(eth::allowance::Approval::max)
        });
        Ok(approvals)
    }
}

impl IntoIterator for Approvals {
    type IntoIter = std::vec::IntoIter<Self::Item>;
    type Item = eth::allowance::Approval;

    fn into_iter(self) -> Self::IntoIter {
        self.0.into_iter()
    }
}
