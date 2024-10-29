use {
    crate::{
        domain::{self, eth},
        infra::blockchain::contracts::{deployment_address, Contracts},
    },
    ethcontract::{dyns::DynWeb3, GasPrice},
};

#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct Manager {
    /// The authenticator contract that decides which solver is allowed to
    /// submit settlements.
    authenticator: contracts::GPv2AllowListAuthentication,
    /// The safe module that is used to provide special role to EOA.
    authenticator_role: contracts::Roles,
    /// The EOA that is allowed to remove solvers.
    authenticator_eoa: ethcontract::Account,
}

impl Manager {
    /// Creates an authenticator which can remove solvers from the allow-list
    pub async fn new(
        web3: DynWeb3,
        network: &network::Network,
        contracts: Contracts,
        authenticator_pk: eth::H256,
    ) -> Self {
        let authenticator_role = contracts::Roles::at(
            &web3,
            deployment_address(contracts::Roles::raw_contract(), network).expect("roles address"),
        );

        Self {
            authenticator: contracts.authenticator().clone(),
            authenticator_role,
            authenticator_eoa: ethcontract::Account::Offline(
                ethcontract::PrivateKey::from_raw(authenticator_pk.0).unwrap(),
                None,
            ),
        }
    }

    /// Fire and forget: Removes solver from the allow-list in the authenticator
    /// contract. This solver will no longer be able to settle.
    #[allow(dead_code)]
    fn remove_solver(&self, solver: domain::eth::Address) {
        let calldata = self
            .authenticator
            .methods()
            .remove_solver(solver.into())
            .tx
            .data
            .expect("missing calldata");
        let authenticator_eoa = self.authenticator_eoa.clone();
        let authenticator_address = self.authenticator.address();
        let authenticator_role = self.authenticator_role.clone();
        tokio::task::spawn(async move {
            // This value comes from the TX posted in the issue: https://github.com/cowprotocol/services/issues/2667
            let mut byte_array = [0u8; 32];
            byte_array[31] = 1;
            authenticator_role
                .methods()
                .exec_transaction_with_role(
                    authenticator_address,
                    0.into(),
                    ethcontract::Bytes(calldata.0),
                    0,
                    ethcontract::Bytes(byte_array),
                    true,
                )
                .from(authenticator_eoa)
                .gas_price(GasPrice::Eip1559 {
                    // These are arbitrary high numbers that should be enough for a tx to be settled
                    // anytime.
                    max_fee_per_gas: 1000.into(),
                    max_priority_fee_per_gas: 5.into(),
                })
                .send()
                .await
                .inspect_err(|err| tracing::error!(?solver, ?err, "failed to remove the solver"))
        });
    }
}
