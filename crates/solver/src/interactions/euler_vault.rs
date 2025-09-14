use {
    contracts::alloy::EulerVault,
    ethcontract::Bytes,
    ethrpc::alloy::conversions::{IntoAlloy, IntoLegacy},
    primitive_types::{H160, U256},
    shared::interaction::{EncodedInteraction, Interaction},
    std::sync::Arc,
};

#[derive(Clone, Debug)]
pub struct EulerVaultInteraction {
    pub max: U256,
    pub receiver: H160,
    // todo: remove Arc
    pub vault: Arc<EulerVault::Instance>,
}

impl Interaction for EulerVaultInteraction {
    fn encode(&self) -> EncodedInteraction {
        let method = self
            .vault
            .skim(self.max.into_alloy(), self.receiver.into_alloy());
        let calldata = method.calldata();
        (
            self.vault.address().into_legacy(),
            0.into(),
            Bytes(calldata.to_vec()),
        )
    }
}
