use {
    contracts::alloy::EulerVault,
    ethcontract::Bytes,
    ethrpc::alloy::conversions::{IntoAlloy, IntoLegacy},
    primitive_types::{H160, U256},
    shared::interaction::{EncodedInteraction, Interaction},
    std::sync::Arc,
};

#[derive(Clone, Debug)]
pub struct EulerVaultDepositInteraction {
    pub deposit_amount: U256,
    pub receiver: H160,
    // todo: remove Arc
    pub vault: Arc<EulerVault::Instance>,
}

impl Interaction for EulerVaultDepositInteraction {
    fn encode(&self) -> EncodedInteraction {
        let method = self
            .vault
            .deposit(self.deposit_amount.into_alloy(), self.receiver.into_alloy());
        let calldata = method.calldata();
        (
            self.vault.address().into_legacy(),
            0.into(),
            Bytes(calldata.to_vec()),
        )
    }
}

#[derive(Clone, Debug)]
pub struct EulerVaultWithdrawInteraction {
    pub redeem_amount: U256,
    pub receiver: H160,
    pub provider: H160,
    // todo: remove Arc
    pub vault: Arc<EulerVault::Instance>,
}

impl Interaction for EulerVaultWithdrawInteraction {
    fn encode(&self) -> EncodedInteraction {
        let method = self.vault.redeem(
            self.redeem_amount.into_alloy(),
            self.receiver.into_alloy(),
            self.provider.into_alloy(),
        );
        let calldata = method.calldata();
        (
            self.vault.address().into_legacy(),
            0.into(),
            Bytes(calldata.to_vec()),
        )
    }
}
