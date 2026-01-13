use {
    super::{OnchainComponents, TestAccount},
    alloy::{
        primitives::{Address, Bytes, U256, keccak256},
        providers::Provider,
        rpc::types::TransactionRequest,
    },
    contracts::alloy::{
        GnosisSafe::{self, GnosisSafe::execTransactionCall},
        GnosisSafeCompatibilityFallbackHandler,
        GnosisSafeProxy,
        GnosisSafeProxyFactory,
    },
    ethrpc::{AlloyProvider, alloy::CallBuilderExt},
    hex_literal::hex,
    model::{
        DomainSeparator,
        order::OrderCreation,
        signature::{Signature, hashed_eip712_message},
    },
    std::marker::PhantomData,
};

pub struct Infrastructure {
    pub factory: GnosisSafeProxyFactory::Instance,
    pub fallback: GnosisSafeCompatibilityFallbackHandler::Instance,
    pub singleton: GnosisSafe::Instance,

    pub provider: AlloyProvider,
}

impl Infrastructure {
    pub async fn new(provider: AlloyProvider) -> Self {
        let singleton = GnosisSafe::Instance::deploy(provider.clone())
            .await
            .unwrap();
        let fallback = GnosisSafeCompatibilityFallbackHandler::Instance::deploy(provider.clone())
            .await
            .unwrap();
        let factory = GnosisSafeProxyFactory::Instance::deploy(provider.clone())
            .await
            .unwrap();

        Self {
            singleton,
            fallback,
            factory,
            provider,
        }
    }

    pub async fn deploy_safe(
        &self,
        owners: Vec<TestAccount>,
        threshold: usize,
    ) -> GnosisSafe::Instance {
        let safe_proxy = GnosisSafeProxy::Instance::deploy_builder(
            self.provider.clone(),
            *self.singleton.address(),
        )
        .deploy()
        .await
        .unwrap();
        let safe = GnosisSafe::Instance::new(safe_proxy, self.provider.clone());

        safe.setup(
            owners.into_iter().map(|owner| owner.address()).collect(),
            U256::from(threshold),
            Address::default(), // delegate call
            Bytes::default(),   // delegate call bytes
            *self.fallback.address(),
            Address::default(), // relayer payment token
            U256::ZERO,         // relayer payment amount
            Address::default(), // relayer address
        )
        .send_and_watch()
        .await
        .unwrap();

        safe
    }
}

/// Wrapper over a deployed Safe.
pub struct Safe {
    chain_id: U256,
    contract: GnosisSafe::Instance,
    owner: TestAccount,
}

impl Safe {
    /// Return a wrapper at the deployed address.
    pub fn deployed(chain_id: U256, contract: GnosisSafe::Instance, owner: TestAccount) -> Self {
        Self {
            chain_id,
            contract,
            owner,
        }
    }

    /// Deploy a Safe with a single owner.
    pub async fn deploy(owner: TestAccount, alloy: AlloyProvider) -> Self {
        // Infrastructure contracts can in principle be reused for any new deployments,
        // but it leads to boilerplate code that we don't need. Redeploying the
        // infrastructure contracts every time should have no appreciable impact in the
        // tests.
        let chain_id = U256::from(alloy.get_chain_id().await.unwrap());
        let infra = Infrastructure::new(alloy).await;
        let contract = infra.deploy_safe(vec![owner.clone()], 1).await;
        Self {
            chain_id,
            contract,
            owner,
        }
    }

    async fn exec_alloy_tx(&self, to: Address, value: U256, calldata: Bytes) {
        self.contract
            .execTransaction(
                to,
                value,
                calldata,
                Default::default(),
                Default::default(),
                Default::default(),
                Default::default(),
                Default::default(),
                Default::default(),
                crate::setup::safe::gnosis_safe_prevalidated_signature(self.owner.address()),
            )
            .from(self.owner.address())
            .send_and_watch()
            .await
            .unwrap();
    }

    pub async fn exec_alloy_call(&self, tx: TransactionRequest) {
        let to = tx.to.unwrap().into_to().unwrap();
        let value = tx.value.unwrap_or_default();
        let data = tx.input.input().unwrap_or_default().to_owned();
        self.exec_alloy_tx(to, value, data).await;
    }

    /// Returns the address of the Safe.
    pub fn address(&self) -> alloy::primitives::Address {
        *self.contract.address()
    }

    /// Returns the next nonce to be used.
    pub async fn nonce(&self) -> alloy::primitives::U256 {
        self.contract.nonce().call().await.unwrap()
    }

    /// Returns a signed transaction ready for execution.
    pub fn sign_transaction(
        &self,
        to: alloy::primitives::Address,
        data: Vec<u8>,
        nonce: alloy::primitives::U256,
    ) -> alloy::contract::CallBuilder<
        &alloy::providers::DynProvider,
        PhantomData<execTransactionCall>,
    > {
        let signature = self.sign({
            // `SafeTx` struct hash computation ported from the Safe Solidity code:
            // <https://etherscan.io/address/0xd9Db270c1B5E3Bd161E8c8503c55cEABeE709552#code#F1#L377>

            let mut buffer = [0_u8; 352];
            buffer[0..32].copy_from_slice(&hex!(
                // `SafeTx` type hash:
                // <https://etherscan.io/address/0xd9Db270c1B5E3Bd161E8c8503c55cEABeE709552#code#F1#L43>
                "bb8310d486368db6bd6f849402fdd73ad53d316b5a4b2644ad6efe0f941286d8"
            ));
            buffer[44..64].copy_from_slice(to.as_slice());
            buffer[96..128].copy_from_slice(keccak256(&data).as_slice());
            nonce.copy_be_bytes_to(&mut buffer[320..352]);

            // Since the [`sign_transaction`] transaction method only accepts
            // a limited number of parameters and defaults to 0 for the others,
            // We can leave the rest of the buffer 0-ed out (as we have 0
            // values for those fields).

            *keccak256(buffer)
        });

        self.contract.execTransaction(
            to,
            Default::default(), // value
            alloy::primitives::Bytes::from(data),
            Default::default(), // operation (= CALL)
            Default::default(), // safe tx gas
            Default::default(), // base gas
            Default::default(), // gas price
            Default::default(), // gas token
            Default::default(), // refund receiver
            alloy::primitives::Bytes::from(signature),
        )
    }

    /// Returns the ERC-1271 signature bytes for the specified message.
    pub fn sign_message(&self, message: &[u8; 32]) -> Vec<u8> {
        self.sign({
            // `SafeMessage` struct hash computation ported from the Safe Solidity code:
            // <https://etherscan.io/address/0xf48f2b2d2a534e402487b3ee7c18c33aec0fe5e4#code#F1#L52>

            let mut buffer = [0_u8; 64];
            buffer[0..32].copy_from_slice(&hex!(
                // `SafeMessage` type hash:
                // <https://etherscan.io/address/0xf48f2b2d2a534e402487b3ee7c18c33aec0fe5e4#code#F1#L14>
                "60b3cbf8b4a223d68d641b3b6ddf9a298e7f33710cf3d3a9d1146b5a6150fbca"
            ));
            buffer[32..64].copy_from_slice(keccak256(message).as_slice());

            *keccak256(buffer)
        })
    }

    pub fn sign_order(&self, order: &mut OrderCreation, onchain: &OnchainComponents) {
        order.signature = Signature::Eip1271(self.order_eip1271_signature(order, onchain));
    }

    pub fn order_eip1271_signature(
        &self,
        order: &OrderCreation,
        onchain: &OnchainComponents,
    ) -> Vec<u8> {
        self.sign_message(&hashed_eip712_message(
            &onchain.contracts().domain_separator,
            &order.data().hash_struct(),
        ))
    }

    /// Returns the domain separator for the Safe.
    fn domain_separator(&self) -> DomainSeparator {
        // Domain separator computation ported from the Safe Solidity code:
        // <https://etherscan.io/address/0xd9Db270c1B5E3Bd161E8c8503c55cEABeE709552#code#F1#L350>

        let mut buffer = [0_u8; 96];
        buffer[0..32].copy_from_slice(&hex!(
            // The domain separator type hash:
            // <https://etherscan.io/address/0xd9Db270c1B5E3Bd161E8c8503c55cEABeE709552#code#F1#L38>
            "47e79534a245952e8b16893a336b85a3d9ea9fa8c573f3d803afb92a79469218"
        ));
        self.chain_id.copy_be_bytes_to(&mut buffer[32..64]);
        buffer[76..96].copy_from_slice(self.contract.address().as_slice());

        DomainSeparator(*keccak256(buffer))
    }

    /// Creates an ECDSA signature with the [`Safe`]'s `owner` and encodes to
    /// bytes in the format expected by the Safe contract.
    fn sign(&self, hash: [u8; 32]) -> Vec<u8> {
        let signature = self.owner.sign_typed_data(&self.domain_separator(), &hash);

        // Signature format specified here:
        // <https://etherscan.io/address/0xd9Db270c1B5E3Bd161E8c8503c55cEABeE709552#code#F11#L20>
        [
            signature.r.as_slice(),
            signature.s.as_slice(),
            &[signature.v],
        ]
        .concat()
    }
}

/// Generate a Safe "pre-validated" signature.
///
/// This is a special "marker" signature that can be used if the account that
/// is executing the transaction is an owner. For single owner safes, this is
/// the easiest way to execute a transaction as it does not involve any ECDSA
/// signing.
///
/// See:
/// - Documentation: <https://docs.gnosis-safe.io/contracts/signatures#pre-validated-signatures>
/// - Code: <https://github.com/safe-global/safe-contracts/blob/c36bcab46578a442862d043e12a83fec41143dec/contracts/GnosisSafe.sol#L287-L291>
pub fn gnosis_safe_prevalidated_signature(
    owner: alloy::primitives::Address,
) -> alloy::primitives::Bytes {
    let mut signature = vec![0; 65];
    signature[12..32].copy_from_slice(owner.as_slice());
    signature[64] = 1;
    alloy::primitives::Bytes::from(signature)
}
