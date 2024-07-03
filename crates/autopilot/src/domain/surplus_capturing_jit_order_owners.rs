use {primitive_types::H160, std::collections::HashSet};

/// Surplus capturing JIT order owners
/// The list of owners is initialized with the values specified in the autopilot
/// configuration file, and it is extended with all the CoW AMM owners which are
/// deployed by the CoW AMM helper contract
pub struct SurplusCapturingJitOrderOwners {
    surplus_capturing_jit_order_owners: HashSet<H160>,
    cow_amm_registry: cow_amm::Registry,
}

impl SurplusCapturingJitOrderOwners {
    pub fn new(
        surplus_capturing_jit_order_owners_configuration: &[H160],
        cow_amm_registry: cow_amm::Registry,
    ) -> Self {
        Self {
            surplus_capturing_jit_order_owners: surplus_capturing_jit_order_owners_configuration
                .iter()
                .cloned()
                .collect(),
            cow_amm_registry,
        }
    }

    pub async fn list_all(&self) -> HashSet<H160> {
        let mut surplus_capturing_jit_order_owners = self
            .cow_amm_registry
            .amms()
            .await
            .into_iter()
            .map(|cow_amm| *cow_amm.address())
            .collect::<HashSet<_>>();
        surplus_capturing_jit_order_owners.extend(self.surplus_capturing_jit_order_owners.clone());
        surplus_capturing_jit_order_owners
    }
}
