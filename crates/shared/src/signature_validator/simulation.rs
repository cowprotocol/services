//! An `eth_call` simulation based ERC-1271 signature verification
//! implementation. This allows orders with ERC-1271 signatures to be used that
//! only get setup as a pre-hook (such as creating a Composable CoW order with a
//! Safe in a pre-interaction).

use {
    super::{SignatureCheck, SignatureValidating, SignatureValidationError},
    crate::ethcontract_error::EthcontractErrorType,
    anyhow::{Context, Result},
    ethcontract::{common::abi::Token, errors::ExecutionError, tokens::Tokenize, Bytes},
    ethrpc::Web3,
    futures::future,
    primitive_types::{H160, U256},
};

pub struct Validator {
    web3: Web3,
    settlement: H160,
    vault_relayer: H160,
}

impl Validator {
    pub fn new(web3: Web3, settlement: H160, vault_relayer: H160) -> Self {
        Self {
            web3,
            settlement,
            vault_relayer,
        }
    }

    async fn simulate(
        &self,
        check: &SignatureCheck,
    ) -> Result<Simulation, SignatureValidationError> {
        // We simulate the signature verification from the Settlement contract's
        // context. This allows us to check:
        // 1. How the pre-interactions would behave as part of the settlement
        // 2. Simulate the actual `isValidSignature` calls that would happen as part of
        //    a settlement
        let signatures =
            contracts::dummy_contract!(contracts::support::Signatures, self.settlement);
        let tx = signatures
            .methods()
            .validate(
                (self.settlement, self.vault_relayer),
                check.signer,
                Bytes(check.hash),
                Bytes(check.signature.clone()),
                check
                    .interactions
                    .iter()
                    .map(|i| (i.target, i.value, Bytes(i.call_data.clone())))
                    .collect(),
            )
            .tx;

        let call = contracts::storage_accessible::call(
            self.settlement,
            contracts::bytecode!(contracts::support::Signatures),
            tx.data.unwrap(),
        );

        let output = self.web3.eth().call(call, None).await?;
        let simulation = Simulation::decode(&output.0)?;

        tracing::trace!(?check, ?simulation, "simulated signature");
        Ok(simulation)
    }
}

#[async_trait::async_trait]
impl SignatureValidating for Validator {
    async fn validate_signatures(
        &self,
        checks: Vec<SignatureCheck>,
    ) -> Vec<Result<(), SignatureValidationError>> {
        future::join_all(checks.into_iter().map(|check| async move {
            self.simulate(&check).await?;
            Ok(())
        }))
        .await
    }

    async fn validate_signature_and_get_additional_gas(
        &self,
        check: SignatureCheck,
    ) -> Result<u64, SignatureValidationError> {
        Ok(self
            .simulate(&check)
            .await?
            .gas_used
            .try_into()
            .unwrap_or(u64::MAX))
    }
}

#[derive(Debug)]
struct Simulation {
    gas_used: U256,
}

impl Simulation {
    fn decode(output: &[u8]) -> Result<Self> {
        let function = contracts::support::Signatures::raw_contract()
            .abi
            .function("validate")
            .unwrap();
        let tokens = function.decode_output(output).context("decode")?;
        let (gas_used,) = Tokenize::from_token(Token::Tuple(tokens))?;

        Ok(Self { gas_used })
    }
}

impl From<web3::Error> for SignatureValidationError {
    fn from(err: web3::Error) -> Self {
        // TODO: This is needed to parse Hardhat revert errors, which
        // `ethcontract` does not support currently.
        if matches!(
            &err,
            web3::Error::Rpc(err)
                if err.message.contains("VM Exception") ||
                    err.message.contains("Transaction reverted")
        ) {
            return Self::Invalid;
        }

        let err = ExecutionError::from(err);
        match EthcontractErrorType::classify(&err) {
            EthcontractErrorType::Contract => Self::Invalid,
            _ => Self::Other(err.into()),
        }
    }
}
