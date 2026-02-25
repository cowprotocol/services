use {
    alloy::{
        eips::eip7702::Authorization,
        network::TransactionBuilder7702,
        primitives::{Address, Bytes, U256, address},
        providers::Provider,
        rpc::types::TransactionRequest,
        signers::{Signer, local::PrivateKeySigner},
        sol_types::SolCall,
    },
    anyhow::{Context, Result},
    contracts::alloy::GnosisSafe::GnosisSafe,
    ethrpc::Web3,
};

/// Builds a Safe "pre-approved" signature for use with `execTransaction`.
///
/// When `v == 1` and `r` equals `msg.sender` (an owner), Safe 1.2.0 accepts
/// the signature without any ECDSA verification or `approveHash` call: being
/// msg.sender is enough.
///
/// Signature layout (65 bytes):
/// - bytes 0-31: `r` — the owner address right-aligned (12 zero bytes + 20
///   address bytes)
/// - bytes 32-63: `s` — zero (not used)
/// - byte 64: `v` — 0x01
/// https://github.com/safe-fndn/safe-smart-account/blob/v1.2.0/contracts/GnosisSafe.sol#L241
fn pre_approved_signature(owner: Address) -> Bytes {
    let mut sig = vec![0u8; 65];
    sig[12..32].copy_from_slice(owner.as_slice());
    sig[64] = 0x01;
    sig.into()
}

/// Safe 1.2.0 contract address used as the EIP-7702 delegation target.
///
/// Version 1.2.0 is preferred for gas efficiency: it does not use guards,
/// which reduces execution overhead compared to newer versions.
pub const SAFE_1_2_0: Address = address!("6851D6fDFAfD08c0295C392436245E5bc78B0185");

/// The EIP-7702 delegation code prefix (`0xef0100`) followed by the 20-byte
/// implementation address. An account delegated to Safe 1.2.0 will have
/// exactly 23 bytes of code: this prefix plus the Safe address.
const EIP7702_DELEGATION_PREFIX: [u8; 3] = [0xef, 0x01, 0x00];

/// Extra gas charged by the Safe `execTransaction` wrapper on top of the
/// underlying settlement gas.
pub const EXEC_TX_GAS_OVERHEAD: u64 = 35_000;

/// A keychain holding multiple private key signers. The first signer is the
/// primary (its address is the on-chain solver address used by CoW Protocol).
/// Additional signers enable concurrent settlement submission via the Safe's
/// `execTransaction` method from different sender addresses.
#[derive(Debug, Clone)]
pub struct Keychain {
    signers: Vec<PrivateKeySigner>,
}

impl Keychain {
    pub fn new(signers: Vec<PrivateKeySigner>) -> Self {
        assert!(
            !signers.is_empty(),
            "keychain must have at least one signer"
        );
        Self { signers }
    }

    /// The primary signer whose address is the registered solver address.
    pub fn primary(&self) -> &PrivateKeySigner {
        &self.signers[0]
    }

    /// Additional signers available for concurrent settlement submission.
    pub fn additional(&self) -> &[PrivateKeySigner] {
        &self.signers[1..]
    }

    /// Address of the primary signer (and the Safe address after EIP-7702
    /// delegation).
    pub fn address(&self) -> Address {
        self.primary().address()
    }

    pub fn has_additional_signers(&self) -> bool {
        self.signers.len() > 1
    }
}

/// Returns whether the account at `address` has EIP-7702 delegation set to
/// Safe 1.2.0.
pub async fn is_delegated_to_safe(provider: &Web3, address: Address) -> Result<bool> {
    let code = provider
        .provider
        .get_code_at(address)
        .await
        .context("failed to get code")?;

    let is_eip7702 = code.len() == 23 && code[..3] == EIP7702_DELEGATION_PREFIX;
    if !is_eip7702 {
        return Ok(false);
    }

    let target = Address::from_slice(&code[3..]);
    if target != SAFE_1_2_0 {
        tracing::warn!(
            %address,
            %target,
            expected = %SAFE_1_2_0,
            "EIP-7702 delegation target mismatch; overriding with Safe 1.2.0"
        );
        return Ok(false);
    }

    Ok(true)
}

/// Ensures the keychain primary address has EIP-7702 delegation pointing to
/// Safe 1.2.0. If not set, submits a type-4 transaction that atomically:
/// 1. Sets the EIP-7702 delegation to Safe 1.2.0
/// 2. Calls `Safe.setup()` on the primary address to initialize it with all
///    keychain signers as owners at a 1-of-N threshold
///
/// This is a no-op when already configured or when only one signer is present.
pub async fn ensure_delegation(
    provider: &Web3,
    keychain: &Keychain,
    chain_id: u64,
    gas_price: alloy::eips::eip1559::Eip1559Estimation,
) -> Result<()> {
    if !keychain.has_additional_signers() {
        return Ok(());
    }

    let primary_address = keychain.address();

    if is_delegated_to_safe(provider, primary_address).await? {
        tracing::info!(%primary_address, "EIP-7702 Safe delegation already configured");
        return Ok(());
    }

    tracing::info!(%primary_address, "setting up EIP-7702 Safe delegation");
    setup_eip7702_safe(provider, keychain, chain_id, gas_price).await?;
    tracing::info!(%primary_address, "EIP-7702 Safe delegation configured");

    Ok(())
}

/// Sends the type-4 (EIP-7702) transaction that sets the delegation and
/// initializes the Safe in a single atomic operation.
/// It is strongly recommended to call this function with flashbots RPC if
/// available for the network.
async fn setup_eip7702_safe(
    provider: &Web3,
    keychain: &Keychain,
    chain_id: u64,
    gas_price: alloy::eips::eip1559::Eip1559Estimation,
) -> Result<()> {
    let primary = keychain.primary();
    let primary_address = primary.address();

    // Register the primary signer so the provider's WalletFiller can sign the
    // type-4 transaction on our behalf.
    provider.wallet.register_signer(primary.clone());

    // Fetch the current nonce. This is used to compute both the
    // transaction nonce and the EIP-7702 authorization nonce.
    let nonce = provider
        .provider
        .get_transaction_count(primary_address)
        .await
        .context("failed to fetch nonce")?;

    // Sign the EIP-7702 authorization for delegating to Safe 1.2.0.
    let auth = Authorization {
        chain_id: U256::from(chain_id),
        address: SAFE_1_2_0,
        nonce: nonce + 1, // The authorization must have a nonce one higher than the tx nonce
    };
    let auth_sig = primary
        .sign_hash(&auth.signature_hash())
        .await
        .context("failed to sign EIP-7702 authorization")?;
    let signed_auth = auth.into_signed(auth_sig);

    // Build Safe `setup` calldata. All keychain signers become owners with a
    // 1-of-N threshold so any single one can execute transactions on the Safe.
    let owners: Vec<Address> = keychain.signers.iter().map(|s| s.address()).collect();
    let setup_calldata = GnosisSafe::setupCall {
        _owners: owners,
        _threshold: U256::from(1),
        to: Address::ZERO,
        data: Bytes::default(),
        fallbackHandler: Address::ZERO,
        paymentToken: Address::ZERO,
        payment: U256::ZERO,
        paymentReceiver: Address::ZERO,
    }
    .abi_encode();

    let tx = TransactionRequest::default()
        .from(primary_address)
        // Call setup on self: EIP-7702 processes authorizations before the
        // tx body executes, so the Safe code is available when setup() runs.
        .to(primary_address)
        .nonce(nonce)
        .max_fee_per_gas(gas_price.max_fee_per_gas)
        .max_priority_fee_per_gas(gas_price.max_priority_fee_per_gas)
        .gas_limit(300_000u64)
        .input(setup_calldata.into())
        .with_authorization_list(vec![signed_auth]);

    let pending_tx = provider
        .provider
        .send_transaction(tx)
        .await
        .context("failed to send EIP-7702 setup transaction")?;

    let receipt = pending_tx
        .get_receipt()
        .await
        .context("failed to get EIP-7702 setup receipt")?;

    anyhow::ensure!(
        receipt.status(),
        "EIP-7702 Safe setup transaction reverted: {:?}",
        receipt.transaction_hash
    );

    Ok(())
}

/// Builds calldata for `Safe.execTransaction(...)` that wraps a settlement tx
/// so it executes from the Safe's address (the primary signer's address).
///
/// The `signer` (msg.sender) must be an owner of the Safe. A pre-approved
/// signature is used: `v=1`, `r=msg.sender`, `s=0`. Safe 1.x accepts this
/// without ECDSA verification when the msg.sender is a registered owner.
pub fn build_exec_transaction_calldata(
    settlement_tx: &crate::domain::eth::Tx,
    signer: &PrivateKeySigner,
) -> Bytes {
    let exec_calldata = GnosisSafe::execTransactionCall {
        to: settlement_tx.to,
        value: settlement_tx.value.0,
        data: settlement_tx.input.clone(),
        operation: 0u8,
        safeTxGas: U256::ZERO,
        baseGas: U256::ZERO,
        gasPrice: U256::ZERO,
        gasToken: Address::ZERO,
        refundReceiver: Address::ZERO,
        signatures: pre_approved_signature(signer.address()),
    }
    .abi_encode();

    exec_calldata.into()
}
