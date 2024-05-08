use {
    super::ChainId,
    crate::domain,
    ethcontract::{dyns::DynWeb3, transaction::TransactionResult},
    primitive_types::H160,
};

#[derive(Debug, Clone)]
pub struct Contracts {
    settlement: contracts::GPv2Settlement,
    weth: contracts::WETH9,
    chainalysis_oracle: Option<contracts::ChainalysisOracle>,

    /// The authenticator contract used for allow-listing solvers to settle.
    authenticator: contracts::GPv2AllowListAuthentication,
    /// The safe module that is used to provide special role to EOA.
    authenticator_role: contracts::Roles,
    /// The EOA that is allowed to add/remove solvers.
    authenticator_eoa: ethcontract::Account,

    /// The domain separator for settlement contract used for signing orders.
    settlement_domain_separator: domain::eth::DomainSeparator,
}

#[derive(Debug, Clone)]
pub struct Addresses {
    pub settlement: Option<H160>,
    pub weth: Option<H160>,
    pub authenticator_eoa: ethcontract::Account,
}

impl Contracts {
    pub async fn new(web3: &DynWeb3, chain: &ChainId, addresses: Addresses) -> Self {
        let address_for = |contract: &ethcontract::Contract, address: Option<H160>| {
            address
                .or_else(|| deployment_address(contract, chain))
                .unwrap()
        };

        let settlement = contracts::GPv2Settlement::at(
            web3,
            address_for(
                contracts::GPv2Settlement::raw_contract(),
                addresses.settlement,
            ),
        );

        let weth = contracts::WETH9::at(
            web3,
            address_for(contracts::WETH9::raw_contract(), addresses.weth),
        );

        let chainalysis_oracle = contracts::ChainalysisOracle::deployed(web3).await.ok();

        let settlement_domain_separator = domain::eth::DomainSeparator(
            settlement
                .domain_separator()
                .call()
                .await
                .expect("domain separator")
                .0,
        );

        let authenticator = contracts::GPv2AllowListAuthentication::at(
            web3,
            settlement
                .authenticator()
                .call()
                .await
                .expect("authenticator address"),
        );

        let authenticator_role = contracts::Roles::at(
            web3,
            deployment_address(contracts::Roles::raw_contract(), chain).expect("roles address"),
        );

        Self {
            settlement,
            weth,
            chainalysis_oracle,
            settlement_domain_separator,
            authenticator,
            authenticator_role,
            authenticator_eoa: addresses.authenticator_eoa,
        }
    }

    pub fn settlement(&self) -> &contracts::GPv2Settlement {
        &self.settlement
    }

    pub fn settlement_domain_separator(&self) -> &domain::eth::DomainSeparator {
        &self.settlement_domain_separator
    }

    pub fn chainalysis_oracle(&self) -> &Option<contracts::ChainalysisOracle> {
        &self.chainalysis_oracle
    }

    pub fn weth(&self) -> &contracts::WETH9 {
        &self.weth
    }

    pub fn authenticator(&self) -> &contracts::GPv2AllowListAuthentication {
        &self.authenticator
    }

    /// Removes solver from the allow-list in the authenticator contract. This
    /// solver will no longer be able to settle.
    pub async fn remove_solver(
        &self,
        solver: domain::eth::Address,
    ) -> Result<TransactionResult, Error> {
        let calldata = self
            .authenticator
            .methods()
            .remove_solver(solver.0)
            .tx
            .data
            .ok_or(Error::SolverRemovalBadCalldata)?;
        self.authenticator_role
            .methods()
            .exec_transaction_with_role(
                self.authenticator.address(),
                0.into(),
                ethcontract::Bytes(calldata.0),
                0,
                ethcontract::Bytes([0; 32]), // populate role
                true,
            )
            .from(self.authenticator_eoa.clone())
            .send()
            .await
            .map_err(Error::SolverRemovalFailed)
    }
}

/// Returns the address of a contract for the specified network, or `None` if
/// there is no known deployment for the contract on that network.
pub fn deployment_address(contract: &ethcontract::Contract, chain: &ChainId) -> Option<H160> {
    Some(contract.networks.get(&chain.to_string())?.address)
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("bad calldata for solver removal")]
    SolverRemovalBadCalldata,
    #[error("failed to remove solver {0}")]
    SolverRemovalFailed(#[from] ethcontract::errors::MethodError),
}
