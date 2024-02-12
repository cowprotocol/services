use {
    super::{OnchainComponents, TestAccount},
    contracts::{
        GnosisSafe,
        GnosisSafeCompatibilityFallbackHandler,
        GnosisSafeProxy,
        GnosisSafeProxyFactory,
    },
    ethcontract::{Bytes, H160, H256, U256},
    hex_literal::hex,
    model::{
        order::OrderCreation,
        signature::{hashed_eip712_message, Signature},
        DomainSeparator,
    },
    shared::ethrpc::Web3,
    web3::{
        signing,
        signing::{Key, SecretKeyRef},
    },
};

macro_rules! tx_safe {
    ($acc:expr, $safe:ident, $call:expr) => {{
        let call = $call;
        $crate::tx!(
            $acc,
            $safe.exec_transaction(
                call.tx.to.unwrap(),
                call.tx.value.unwrap_or_default(),
                ::ethcontract::Bytes(call.tx.data.unwrap_or_default().0),
                Default::default(),
                Default::default(),
                Default::default(),
                Default::default(),
                Default::default(),
                Default::default(),
                $crate::setup::safe::gnosis_safe_prevalidated_signature($acc.address()),
            )
        );
    }};
}

pub struct Infrastructure {
    pub factory: GnosisSafeProxyFactory,
    pub fallback: GnosisSafeCompatibilityFallbackHandler,
    pub singleton: GnosisSafe,
    web3: Web3,
}

impl Infrastructure {
    pub async fn new(web3: &Web3) -> Self {
        let singleton = GnosisSafe::builder(web3).deploy().await.unwrap();
        let fallback = GnosisSafeCompatibilityFallbackHandler::builder(web3)
            .deploy()
            .await
            .unwrap();
        let factory = GnosisSafeProxyFactory::builder(web3)
            .deploy()
            .await
            .unwrap();
        Self {
            web3: web3.clone(),
            singleton,
            fallback,
            factory,
        }
    }

    pub async fn deploy_safe(&self, owners: Vec<H160>, threshold: usize) -> GnosisSafe {
        let safe_proxy = GnosisSafeProxy::builder(&self.web3, self.singleton.address())
            .deploy()
            .await
            .unwrap();
        let safe = GnosisSafe::at(&self.web3, safe_proxy.address());
        safe.setup(
            owners,
            threshold.into(),
            H160::default(),  // delegate call
            Bytes::default(), // delegate call bytes
            self.fallback.address(),
            H160::default(), // relayer payment token
            0.into(),        // relayer payment amount
            H160::default(), // relayer address
        )
        .send()
        .await
        .unwrap();
        safe
    }
}

/// Wrapper over a deployed Safe.
pub struct Safe {
    chain_id: U256,
    contract: GnosisSafe,
    owner: TestAccount,
}

impl Safe {
    /// Return a wrapper at the deployed address.
    pub fn deployed(chain_id: U256, contract: GnosisSafe, owner: TestAccount) -> Self {
        Self {
            chain_id,
            contract,
            owner,
        }
    }

    /// Deploy a Safe with a single owner.
    pub async fn deploy(owner: TestAccount, web3: &Web3) -> Self {
        // Infrastructure contracts can in principle be reused for any new deployments,
        // but it leads to boilerplate code that we don't need. Redeploying the
        // infrastructure contracts every time should have no appreciable impact in the
        // tests.
        let infra = Infrastructure::new(web3).await;
        let chain_id = web3.eth().chain_id().await.unwrap();
        let contract = infra.deploy_safe(vec![owner.address()], 1).await;
        Self {
            chain_id,
            contract,
            owner,
        }
    }

    pub async fn exec_call<T: ethcontract::tokens::Tokenize>(
        &self,
        tx: ethcontract::dyns::DynMethodBuilder<T>,
    ) {
        let contract = &self.contract;
        tx_safe!(self.owner.account(), contract, tx);
    }

    /// Returns the address of the Safe.
    pub fn address(&self) -> H160 {
        self.contract.address()
    }

    /// Returns a signed transaction ready for execution.
    pub fn sign_transaction(
        &self,
        to: H160,
        data: Vec<u8>,
        nonce: U256,
    ) -> ethcontract::dyns::DynMethodBuilder<bool> {
        let signature = self.sign({
            // `SafeTx` struct hash computation ported from the Safe Solidity code:
            // <https://etherscan.io/address/0xd9Db270c1B5E3Bd161E8c8503c55cEABeE709552#code#F1#L377>

            let mut buffer = [0_u8; 352];
            buffer[0..32].copy_from_slice(&hex!(
                // `SafeTx` type hash:
                // <https://etherscan.io/address/0xd9Db270c1B5E3Bd161E8c8503c55cEABeE709552#code#F1#L43>
                "bb8310d486368db6bd6f849402fdd73ad53d316b5a4b2644ad6efe0f941286d8"
            ));
            buffer[44..64].copy_from_slice(to.as_bytes());
            buffer[96..128].copy_from_slice(&signing::keccak256(&data));
            nonce.to_big_endian(&mut buffer[320..352]);

            // Since the [`sign_transaction`] transaction method only accepts
            // a limited number of parameters and defaults to 0 for the others,
            // We can leave the rest of the buffer 0-ed out (as we have 0
            // values for those fields).

            signing::keccak256(&buffer)
        });

        self.contract.exec_transaction(
            to,
            Default::default(), // value
            Bytes(data),
            Default::default(), // operation (= CALL)
            Default::default(), // safe tx gas
            Default::default(), // base gas
            Default::default(), // gas price
            Default::default(), // gas token
            Default::default(), // refund receiver
            Bytes(signature),
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
            buffer[32..64].copy_from_slice(&signing::keccak256(message));

            signing::keccak256(&buffer)
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
        self.chain_id.to_big_endian(&mut buffer[32..64]);
        buffer[76..96].copy_from_slice(self.contract.address().as_bytes());

        DomainSeparator(signing::keccak256(&buffer))
    }

    /// Creates an ECDSA signature with the [`Safe`]'s `owner` and encodes to
    /// bytes in the format expected by the Safe contract.
    fn sign(&self, hash: [u8; 32]) -> Vec<u8> {
        let signature = self.owner.sign_typed_data(&self.domain_separator(), &hash);

        // Signature format specified here:
        // <https://etherscan.io/address/0xd9Db270c1B5E3Bd161E8c8503c55cEABeE709552#code#F11#L20>
        [
            signature.r.as_bytes(),
            signature.s.as_bytes(),
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
pub fn gnosis_safe_prevalidated_signature(owner: H160) -> Bytes<Vec<u8>> {
    let mut signature = vec![0; 65];
    signature[12..32].copy_from_slice(owner.as_bytes());
    signature[64] = 1;
    Bytes(signature)
}

/// Generate an owner signature for EIP-1271.
///
/// The Gnosis Safe uses off-chain ECDSA signatures from its owners as the
/// signature bytes when validating EIP-1271 signatures. Specifically, it
/// expects a signed EIP-712 `SafeMessage(bytes message)` (where `message` is
/// the 32-byte hash of the data being verified).
///
/// See:
/// - Code: <https://github.com/safe-global/safe-contracts/blob/c36bcab46578a442862d043e12a83fec41143dec/contracts/handler/CompatibilityFallbackHandler.sol#L66-L70>
pub async fn gnosis_safe_eip1271_signature(
    key: SecretKeyRef<'_>,
    safe: &GnosisSafe,
    message_hash: H256,
) -> Vec<u8> {
    let handler =
        GnosisSafeCompatibilityFallbackHandler::at(&safe.raw_instance().web3(), safe.address());

    let signing_hash = handler
        .get_message_hash(Bytes(message_hash.as_bytes().to_vec()))
        .call()
        .await
        .unwrap();

    let signature = key.sign(&signing_hash.0, None).unwrap();

    let mut bytes = vec![0u8; 65];
    bytes[0..32].copy_from_slice(signature.r.as_bytes());
    bytes[32..64].copy_from_slice(signature.s.as_bytes());
    bytes[64] = signature.v as _;

    bytes
}
