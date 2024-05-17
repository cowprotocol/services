use {
    crate::{
        domain::{self, eth},
        infra::blockchain::{contracts::deployment_address, ChainId},
    },
    ethcontract::dyns::DynWeb3,
    primitive_types::H160,
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

pub struct Addresses {
    pub settlement: Option<eth::Address>,
    pub authenticator_eoa: eth::H256,
}

impl Authenticator {
    /// Creates an authenticator which can remove solvers from the allow-list
    pub async fn new(web3: DynWeb3, chain: ChainId, addresses: Addresses) -> Self {
        let address_for = |contract: &ethcontract::Contract, address: Option<H160>| {
            address
                .or_else(|| deployment_address(contract, &chain))
                .unwrap()
        };

        let settlement = contracts::GPv2Settlement::at(
            &web3,
            address_for(
                contracts::GPv2Settlement::raw_contract(),
                addresses.settlement.map(Into::into),
            ),
        );

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
            authenticator_eoa: ethcontract::Account::Offline(
                ethcontract::PrivateKey::from_raw(addresses.authenticator_eoa.0).unwrap(),
                None,
            ),
        }
    }

    /// Fire and forget: Removes solver from the allow-list in the authenticator
    /// contract. This solver will no longer be able to settle.
    #[allow(dead_code)]
    async fn remove_solver(&self, solver: domain::eth::Address) -> Result<(), Error> {
        let calldata = self
            .authenticator
            .methods()
            .remove_solver(solver.into())
            .tx
            .data
            .ok_or(Error::SolverRemovalBadCalldata)?;
        let authenticator_eoa = self.authenticator_eoa.clone();
        let authenticator_address = self.authenticator.address();
        let authenticator_role = self.authenticator_role.clone();
        tokio::task::spawn(async move {
            authenticator_role
                .methods()
                .exec_transaction_with_role(
                    authenticator_address,
                    0.into(),
                    ethcontract::Bytes(calldata.0),
                    0,
                    ethcontract::Bytes([0; 32]), // @TODO: populate role
                    true,
                )
                .from(authenticator_eoa)
                .send()
                .await
                .map_err(Error::SolverRemovalFailed)
        });
        Ok(())
    }
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("bad calldata for solver removal")]
    SolverRemovalBadCalldata,
    #[error("failed to remove solver {0}")]
    SolverRemovalFailed(#[from] ethcontract::errors::MethodError),
}
