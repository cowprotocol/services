//! Module including logic for signing, and encoding a settlement over the
//! `SolverTrampoline` contract.
//!
//! This allows settlement transactions to be executed permissionlessly by any
//! relayer using signed settlement calldata.

use crate::{settlement::Settlement, settlement_simulation::settle_method_builder};
use anyhow::{bail, Result};
use contracts::{GPv2Settlement, SolverTrampoline};
use ethcontract::{Account, Bytes, U256};
use hex_literal::hex;
use model::{
    signature::{EcdsaSignature, EcdsaSigningScheme},
    DomainSeparator,
};
use shared::{
    gelato_api::GelatoCall,
    http_solver::model::InternalizationStrategy::SkipInternalizableInteraction,
};
use web3::signing::{self, SecretKeyRef};

pub struct Trampoline {
    chain_id: u64,
    contracts: Contracts,
    domain_separator: DomainSeparator,
}

struct Contracts {
    settlement: GPv2Settlement,
    trampoline: SolverTrampoline,
}

impl Trampoline {
    /// Creates a new Trampoline signer.
    pub async fn initialize(settlement: GPv2Settlement) -> Result<Self> {
        let web3 = settlement.raw_instance().web3();
        let chain_id = web3.eth().chain_id().await?.as_u64();
        let trampoline = SolverTrampoline::deployed(&web3).await?;
        let domain_separator = DomainSeparator(trampoline.domain_separator().call().await?.0);

        Ok(Self {
            chain_id,
            contracts: Contracts {
                settlement,
                trampoline,
            },
            domain_separator,
        })
    }

    /// Prepares a Gelato relayer call.
    pub async fn prepare_call(
        &self,
        account: &Account,
        settlement: &Settlement,
    ) -> Result<GelatoCall> {
        let key = match account {
            Account::Offline(key, _) => SecretKeyRef::new(key),
            _ => bail!("offline account required for relayed submission"),
        };

        let calldata = settle_method_builder(
            &self.contracts.settlement,
            settlement.clone().encode(SkipInternalizableInteraction),
            account.clone(),
        )
        .tx
        .data
        .unwrap()
        .0;

        let nonce = self.contracts.trampoline.nonce().call().await?;
        let signature = self.sign(key, &calldata, nonce);

        let relay = self
            .contracts
            .trampoline
            .settle(
                Bytes(calldata),
                Bytes(signature.r.0),
                Bytes(signature.s.0),
                signature.v,
            )
            .from(account.clone());

        Ok(GelatoCall {
            chain_id: self.chain_id,
            target: relay.tx.to.unwrap(),
            data: relay.tx.data.unwrap().0,
            ..Default::default()
        })
    }

    fn sign(&self, key: SecretKeyRef, calldata: &[u8], nonce: U256) -> EcdsaSignature {
        // Solver trampoline solutions are signed with EIP-712 using the
        // following message type:
        //
        // ```
        // Solution(
        //   bytes solution,
        //   uint256 nonce
        // )
        // ```
        //
        // This is just the pre-computed "type-hash" that is needed for EIP-712
        // message hashing and signing.
        //
        // <https://eips.ethereum.org/EIPS/eip-712>
        // <https://goerli.etherscan.io/address/0xd29ae121ad58479c9eb8c4f235c618fcf42ecba0#code>
        const SOLUTION_TYPE_HASH: [u8; 32] =
            hex!("7014cf19af88c8fc5ee7e2c42ed71b7b8f804064f82f63de38c0d59473ce7d7c");

        let struct_hash = signing::keccak256(&{
            let mut buffer = [0_u8; 96];
            buffer[0..32].copy_from_slice(&SOLUTION_TYPE_HASH);
            buffer[32..64].copy_from_slice(&signing::keccak256(calldata));
            nonce.to_big_endian(&mut buffer[64..96]);
            buffer
        });

        EcdsaSignature::sign(
            EcdsaSigningScheme::Eip712,
            &self.domain_separator,
            &struct_hash,
            key,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use shared::{
        addr,
        ethrpc::{create_env_test_transport, Web3},
        tenderly_api::{SimulationRequest, TenderlyHttpApi},
    };
    use std::env;

    #[ignore]
    #[tokio::test]
    async fn trampoline_transaction() {
        let web3 = Web3::new(create_env_test_transport());
        let tenderly = TenderlyHttpApi::test_from_env();

        let settlement = GPv2Settlement::deployed(&web3).await.unwrap();
        let trampoline = Trampoline::initialize(settlement).await.unwrap();

        let solver = Account::Offline(env::var("SOLVER_ACCOUNT").unwrap().parse().unwrap(), None);
        let settlement = Settlement::default();

        let call = trampoline.prepare_call(&solver, &settlement).await.unwrap();
        let simulation = tenderly
            .simulate(SimulationRequest {
                network_id: call.chain_id.to_string(),
                from: addr!("1337133713371337133713371337133713371337"), // NOT a solver
                to: call.target,
                input: call.data,
                save: Some(true),
                save_if_fails: Some(true),
                ..Default::default()
            })
            .await
            .unwrap();

        assert!(simulation.transaction.status, "simulation failed");
    }
}
