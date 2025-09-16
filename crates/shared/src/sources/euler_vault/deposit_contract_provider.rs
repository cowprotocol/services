use {alloy::primitives::Address, contracts::alloy::EulerPerspective::EulerPerspective::{self, EulerPerspectiveInstance}, ethcontract::H160, model::TokenPair, std::any::Any, web3::signing::keccak256};

#[derive(Debug)]
pub struct DepositContractProvider<T: alloy::providers::Provider> {
    perspective_contract: EulerPerspectiveInstance<T>
}

impl<T: alloy::providers::Provider> DepositContractProvider<T> {
    pub async fn get_all_deposit_contracts(&self, pair: &TokenPair) -> Result<Vec<Address>, alloy::contract::Error> {
        self.perspective_contract.verifiedArray().call().await
    }
}
