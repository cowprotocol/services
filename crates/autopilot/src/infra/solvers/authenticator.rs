use {
    crate::{
        domain,
        infra::blockchain::{contracts::deployment_address, ChainId},
    },
    ethcontract::{dyns::DynWeb3, transaction::TransactionResult},
};

#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct Authenticator {
    /// The authenticator contract used for allow-listing solvers to settle.
    authenticator: contracts::GPv2AllowListAuthentication,
    /// The safe module that is used to provide special role to EOA.
    authenticator_role: contracts::Roles,
    /// The EOA that is allowed to add/remove solvers.
    authenticator_eoa: ethcontract::Account,
}

impl Authenticator {
    /// Creates an authenticator which can remove solvers from the allow-list
    pub async fn new(
        web3: DynWeb3,
        chain: ChainId,
        settlement: contracts::GPv2Settlement,
        authenticator_eoa: ethcontract::Account,
    ) -> Self {
        let authenticator = contracts::GPv2AllowListAuthentication::at(
            &web3,
            settlement
                .authenticator()
                .call()
                .await
                .expect("authenticator address"),
        );

        let authenticator_role = contracts::Roles::at(
            &web3,
            deployment_address(contracts::Roles::raw_contract(), &chain).expect("roles address"),
        );

        Self {
            authenticator,
            authenticator_role,
            authenticator_eoa,
        }
    }

    /// Removes solver from the allow-list in the authenticator contract. This
    /// solver will no longer be able to settle.
    #[allow(dead_code)]
    async fn remove_solver(
        &self,
        solver: domain::eth::Address,
    ) -> Result<TransactionResult, Error> {
        let calldata = self
            .authenticator
            .methods()
            .remove_solver(solver.into())
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
                ethcontract::Bytes([0; 32]), // @TODO: populate role
                true,
            )
            .from(self.authenticator_eoa.clone())
            .send()
            .await
            .map_err(Error::SolverRemovalFailed)
    }
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("bad calldata for solver removal")]
    SolverRemovalBadCalldata,
    #[error("failed to remove solver {0}")]
    SolverRemovalFailed(#[from] ethcontract::errors::MethodError),
}
