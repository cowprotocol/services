//! An `eth_call` simulation based ERC-1271 signature verification
//! implementation. This allows orders with ERC-1271 signatures to be used that
//! only get setup as a pre-hook (such as creating a Composable CoW order with a
//! Safe in a pre-interaction).

use {
    super::{SignatureCheck, SignatureValidating, SignatureValidationError},
    anyhow::Result,
    contracts::{ERC1271SignatureValidator, errors::EthcontractErrorType},
    ethcontract::{Account, Bytes, PrivateKey},
    ethrpc::Web3,
    futures::future,
    primitive_types::{H160, U256},
    std::sync::LazyLock,
    tracing::instrument,
};

pub struct Validator(Box<dyn Simulator>);

struct EvmValidator {
    signatures: contracts::support::Signatures,
    settlement: H160,
    vault_relayer: H160,
    web3: Web3,
}

struct ZkSyncValidator {
    signatures: contracts::support::Signatures,
    settlement: contracts::GPv2Settlement,
    vault_relayer: H160,
    web3: Web3,
}

#[async_trait::async_trait]
trait Simulator: Send + Sync {
    async fn simulate(
        &self,
        check: &SignatureCheck,
    ) -> Result<Simulation, SignatureValidationError>;

    fn web3(&self) -> &Web3;
}

#[async_trait::async_trait]
impl Simulator for Validator {
    #[instrument(skip_all, fields(interactions_len = check.interactions.len()))]
    async fn simulate(
        &self,
        check: &SignatureCheck,
    ) -> Result<Simulation, SignatureValidationError> {
        self.0.simulate(check).await
    }

    fn web3(&self) -> &Web3 {
        self.0.web3()
    }
}

#[async_trait::async_trait]
impl Simulator for EvmValidator {
    #[instrument(skip_all, fields(interactions_len = check.interactions.len()))]
    async fn simulate(
        &self,
        check: &SignatureCheck,
    ) -> Result<Simulation, SignatureValidationError> {
        // memoize byte code to not hex-decode it on every call
        static BYTECODE: LazyLock<web3::types::Bytes> =
            LazyLock::new(|| contracts::bytecode!(contracts::support::Signatures));

        // We simulate the signature verification from the Settlement contract's
        // context. This allows us to check:
        // 1. How the pre-interactions would behave as part of the settlement
        // 2. Simulate the actual `isValidSignature` calls that would happen as part of
        //    a settlement
        let result = contracts::storage_accessible::simulate(
            BYTECODE.clone(),
            self.signatures.methods().validate(
                (self.settlement, self.vault_relayer),
                check.signer,
                Bytes(check.hash),
                Bytes(check.signature.clone()),
                check
                    .interactions
                    .iter()
                    .map(|i| (i.target, i.value, Bytes(i.call_data.clone())))
                    .collect(),
            ),
        )
        .await;

        tracing::trace!(?check, ?result, "simulated signature");
        Ok(Simulation { gas_used: result? })
    }

    fn web3(&self) -> &Web3 {
        &self.web3
    }
}

#[async_trait::async_trait]
impl Simulator for ZkSyncValidator {
    #[instrument(skip_all, fields(interactions_len = check.interactions.len()))]
    async fn simulate(
        &self,
        check: &SignatureCheck,
    ) -> Result<Simulation, SignatureValidationError> {
        static SIMULATION_ACCOUNT: LazyLock<Account> = LazyLock::new(|| {
            PrivateKey::from_hex_str(
                "0000000000000000000000000000000000000000000000000000000000018894",
            )
            .map(|pk| Account::Offline(pk, None))
            .expect("valid simulation private key")
        });
        // We simulate the signature verification from the Settlement contract's
        // context. This allows us to check:
        // 1. How the pre-interactions would behave as part of the settlement
        // 2. Simulate the actual `isValidSignature` calls that would happen as part of
        //    a settlement
        let validate_call = self.signatures.methods().validate(
            (self.settlement.address(), self.vault_relayer),
            check.signer,
            Bytes(check.hash),
            Bytes(check.signature.clone()),
            check
                .interactions
                .iter()
                .map(|i| (i.target, i.value, Bytes(i.call_data.clone())))
                .collect(),
        );
        let gas_cost_call = self
            .settlement
            .simulate_delegatecall(
                self.signatures.address(),
                Bytes(validate_call.tx.data.unwrap_or_default().0),
            )
            .from(SIMULATION_ACCOUNT.clone());
        let result = gas_cost_call
            .tx
            .estimate_gas()
            .await
            .map_err(|err| SignatureValidationError::Other(err.into()));

        tracing::trace!(?check, ?result, "simulated signature");
        Ok(Simulation { gas_used: result? })
    }

    fn web3(&self) -> &Web3 {
        &self.web3
    }
}

impl Validator {
    /// The result returned from `isValidSignature` if the signature is correct
    const IS_VALID_SIGNATURE_MAGIC_BYTES: &'static str = "1626ba7e";

    pub async fn new(
        web3: &Web3,
        settlement: contracts::GPv2Settlement,
        vault_relayer: H160,
    ) -> Result<Self> {
        let web3 = ethrpc::instrumented::instrument_with_label(web3, "signatureValidation".into());
        let chain_id = web3.eth().chain_id().await?.low_u32();
        let simulator: Box<dyn Simulator> = match chain_id {
            // ZkSync-based chains
            232 | 324 => Box::new(ZkSyncValidator::new(&web3, settlement, vault_relayer).await?),
            _ => Box::new(EvmValidator::new(
                &web3,
                settlement.address(),
                vault_relayer,
            )),
        };

        Ok(Self(simulator))
    }

    /// Simulate isValidSignature for the cases in which the order does not have
    /// pre-interactions
    pub async fn simulate_without_pre_interactions(
        &self,
        check: &SignatureCheck,
    ) -> Result<(), SignatureValidationError> {
        // Since there are no interactions (no dynamic conditions / complex pre-state
        // change), the order's validity can be directly determined by whether
        // the signature matches the expected hash of the order data, checked
        // with isValidSignature method called on the owner's contract
        let contract = ERC1271SignatureValidator::at(self.web3(), check.signer);
        let magic_bytes = contract
            .methods()
            .is_valid_signature(Bytes(check.hash), Bytes(check.signature.clone()))
            .call()
            .await
            .map(|value| hex::encode(value.0))?;

        if magic_bytes != Self::IS_VALID_SIGNATURE_MAGIC_BYTES {
            return Err(SignatureValidationError::Invalid);
        }

        Ok(())
    }
}

impl ZkSyncValidator {
    pub async fn new(
        web3: &Web3,
        settlement: contracts::GPv2Settlement,
        vault_relayer: H160,
    ) -> Result<Self> {
        Ok(Self {
            signatures: contracts::support::Signatures::deployed(web3).await?,
            settlement,
            vault_relayer,
            web3: web3.clone(),
        })
    }
}

impl EvmValidator {
    pub fn new(web3: &Web3, settlement: H160, vault_relayer: H160) -> Self {
        Self {
            signatures: contracts::support::Signatures::at(web3, settlement),
            settlement,
            vault_relayer,
            web3: web3.clone(),
        }
    }
}

#[async_trait::async_trait]
impl SignatureValidating for Validator {
    #[instrument(skip_all)]
    async fn validate_signatures(
        &self,
        checks: Vec<SignatureCheck>,
    ) -> Vec<Result<(), SignatureValidationError>> {
        future::join_all(checks.into_iter().map(|check| async move {
            if check.interactions.is_empty() {
                self.simulate_without_pre_interactions(&check).await?;
            } else {
                self.simulate(&check).await?;
            }
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

impl From<ethcontract::errors::MethodError> for SignatureValidationError {
    fn from(err: ethcontract::errors::MethodError) -> Self {
        match EthcontractErrorType::classify(&err) {
            EthcontractErrorType::Contract => Self::Invalid,
            _ => Self::Other(err.into()),
        }
    }
}
