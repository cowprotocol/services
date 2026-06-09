use {
    super::{Config, Solver},
    crate::infra::blockchain::Ethereum,
    alloy::{
        eips::eip7702::{Authorization, SignedAuthorization},
        network::{ReceiptResponse, TransactionBuilder7702, TxSigner},
        primitives::{Address, B256, Bytes, U256, address},
        providers::Provider,
        rpc::types::TransactionRequest,
        sol_types::SolConstructor,
    },
    anyhow::Context,
    contracts::Solver7702Delegate::Solver7702Delegate,
    hex_literal::hex,
    std::{collections::HashSet, time::Duration},
    tracing::instrument,
};

/// EIP-7702 delegation prefix stored as account code prefix. If you call
/// eth_getCode on a delegated EOA, instead of getting empty bytes (normal EOA),
/// you get 0xef0100<20-byte contract address>.
pub const DELEGATION_PREFIX: [u8; 3] = [0xef, 0x01, 0x00];
const DELEGATION_CODE_LEN: usize = DELEGATION_PREFIX.len() + Address::len_bytes();
/// The maximum number of approved callers allowed by the Solver7702Delegate
/// ABI.
pub const MAX_APPROVED_CALLERS: usize = 5;
// Arachnid's deterministic-deployment-proxy. It is deployed at this same
// address on many EVM chains. Sending 32-byte salt || init code to it deploys
// that init code with CREATE2. We use it to derive and deploy the exact
// Solver7702Delegate address from this proxy address, zero salt, and init code.
pub const CREATE2_DEPLOYER: Address = address!("4e59b44847b379578588920cA78FbF26c0B4956C");
const CREATE2_DEPLOYER_CODE: &[u8] = &hex!(
    "7fffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffe03601600081602082378035828234f58015156039578182fd5b8082525050506014600cf3"
);
// The ordered caller slots and zero salt are part of the CREATE2 input, so the
// same caller set keeps resolving to the same delegate target.
pub const CREATE2_SALT: B256 = B256::ZERO;

/// Ensure EIP-7702 delegate deployment and solver delegation are set up for all
/// solvers with parallel submission accounts. Called once at driver startup.
///
/// # Errors
/// - A solver has `submission-accounts`, but its main account is read-only and
///   cannot sign the EIP-7702 authorization.
/// - The deterministic CREATE2 deployer is missing or has unexpected code.
/// - The solver EOA already delegates to another target, or has non-delegation
///   code.
/// - The EIP-7702 authorization lands but the on-chain code does not reflect
///   the expected delegate, for example because a concurrent tx shifted the
///   nonce.
/// - Any underlying RPC error while fetching code, chain id, nonces, sending a
///   tx, or waiting for a receipt.
#[instrument(name = "setup_eip7702", skip_all)]
pub async fn setup(solvers: &[Solver], eth: &Ethereum) -> anyhow::Result<()> {
    for solver in solvers {
        let config = solver.config();
        if config.submission_accounts.is_empty() {
            continue;
        }

        // Register solver + submission accounts with the main wallet so we can
        // send transactions via the provider during setup.
        let web3 = eth.web3();
        web3.wallet.register_signer(config.account.clone());
        for acc in &config.submission_accounts {
            web3.wallet.register_signer(acc.clone());
        }

        let submission_addresses = config
            .submission_accounts
            .iter()
            .map(TxSigner::address)
            .collect::<Vec<_>>();
        let (delegate, approved_callers, init_code) = delegate_deployment(&submission_addresses)?;

        setup_solver(config, delegate, &approved_callers, &init_code, eth).await?;
    }
    Ok(())
}

pub fn delegate_address(callers: &[Address]) -> anyhow::Result<Address> {
    Ok(delegate_deployment(callers)?.0)
}

fn delegate_deployment(
    callers: &[Address],
) -> anyhow::Result<(Address, [Address; MAX_APPROVED_CALLERS], Bytes)> {
    anyhow::ensure!(
        callers.len() <= MAX_APPROVED_CALLERS,
        "Solver7702Delegate supports at most {MAX_APPROVED_CALLERS} submission accounts"
    );
    anyhow::ensure!(
        callers.iter().all(|caller| *caller != Address::ZERO),
        "submission accounts cannot include the zero address"
    );
    let mut seen = HashSet::with_capacity(callers.len());
    anyhow::ensure!(
        callers.iter().all(|caller| seen.insert(*caller)),
        "submission accounts must be unique"
    );

    let mut approved_callers = [Address::ZERO; MAX_APPROVED_CALLERS];
    approved_callers[..callers.len()].copy_from_slice(callers);

    anyhow::ensure!(
        !Solver7702Delegate::BYTECODE.is_empty(),
        "Solver7702Delegate creation bytecode is missing"
    );
    let init_code = Solver7702Delegate::BYTECODE
        .iter()
        .chain(&SolConstructor::abi_encode(
            &Solver7702Delegate::constructorCall {
                approvedCallers: approved_callers,
            },
        ))
        .copied()
        .collect::<Bytes>();

    // The submission account order is part of the constructor args, so changing
    // TOML order changes the CREATE2 delegate address.
    let target = CREATE2_DEPLOYER.create2_from_code(CREATE2_SALT, &init_code);

    Ok((target, approved_callers, init_code))
}

#[instrument(skip_all, fields(delegate = ?delegate))]
async fn setup_solver(
    config: &Config,
    delegate: Address,
    approved_callers: &[Address; MAX_APPROVED_CALLERS],
    init_code: &Bytes,
    eth: &Ethereum,
) -> anyhow::Result<()> {
    let provider = &eth.web3().provider;
    let solver_address = config.account.address();
    let delegate_code = provider
        .get_code_at(delegate)
        .await
        .context("reading Solver7702Delegate code")?;
    let solver_code = provider
        .get_code_at(solver_address)
        .await
        .context("reading solver EOA code")?;

    let delegate_missing = delegate_code.is_empty();
    match (DelegationStatus::from_code(&solver_code), delegate_missing) {
        // The solver EOA already delegates somewhere else. Do not silently undo
        // a manual change or incident response action.
        (DelegationStatus::DelegatedTo(target), _) if target != delegate => anyhow::bail!(
            "solver '{}': solver EOA {:?} already delegates to {:?}, expected {:?}; refusing to \
             re-delegate automatically on startup. Clear the existing delegation manually if this \
             is intentional.",
            config.name,
            solver_address,
            target,
            delegate,
        ),
        // The solver account has code that is not an EIP-7702 delegation. This
        // is unexpected for an EOA, so fail instead of overwriting it.
        (DelegationStatus::OtherCode, _) => anyhow::bail!(
            "solver '{}': solver EOA {:?} has non-empty code that is not an EIP-7702 delegation; \
             refusing to overwrite it on startup",
            config.name,
            solver_address,
        ),
        // A previous setup attempt may have set the delegation but failed
        // before CREATE2 deployment succeeded. The EOA already points to the
        // right counterfactual address, so only deploy the missing code.
        (DelegationStatus::DelegatedTo(_), true) => {
            deploy_delegate(
                config,
                delegate,
                approved_callers,
                init_code,
                DeploymentMode::DeployOnly,
                eth,
            )
            .await
        }
        // Everything is already set up.
        (DelegationStatus::DelegatedTo(_), false) => {
            tracing::info!(
                solver = %config.name,
                delegate = ?delegate,
                "solver EOA already delegates to Solver7702Delegate"
            );
            Ok(())
        }
        // Fresh setup: neither the EOA delegation nor the CREATE2 delegate
        // exists, so deploy and delegate in one transaction.
        (DelegationStatus::Empty, true) => {
            deploy_delegate(
                config,
                delegate,
                approved_callers,
                init_code,
                DeploymentMode::DeployAndDelegate,
                eth,
            )
            .await
        }
        // The delegate was deployed already, but this solver EOA has no
        // delegation yet. This can happen when another startup process deployed
        // the shared CREATE2 target first, so warn and set delegation now.
        (DelegationStatus::Empty, false) => {
            tracing::warn!(
                solver = %config.name,
                solver_eoa = ?solver_address,
                delegate = ?delegate,
                "solver EOA has no EIP-7702 delegation but expected delegate already exists; \
                 setting delegation"
            );
            setup_delegation(config, delegate, eth).await
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DeploymentMode {
    DeployOnly,
    DeployAndDelegate,
}

impl DeploymentMode {
    fn includes_delegation(self) -> bool {
        matches!(self, Self::DeployAndDelegate)
    }
}

#[instrument(skip_all, fields(delegate = ?delegate))]
async fn deploy_delegate(
    config: &Config,
    delegate: Address,
    approved_callers: &[Address; MAX_APPROVED_CALLERS],
    init_code: &Bytes,
    mode: DeploymentMode,
    eth: &Ethereum,
) -> anyhow::Result<()> {
    let provider = &eth.web3().provider;
    let deployer_code = provider
        .get_code_at(CREATE2_DEPLOYER)
        .await
        .context("reading CREATE2 deployer code")?;
    anyhow::ensure!(
        deployer_code.as_ref() == CREATE2_DEPLOYER_CODE,
        "CREATE2 deployer {CREATE2_DEPLOYER:?} has unexpected code",
    );

    let tx_sender = match mode {
        DeploymentMode::DeployOnly => config
            .submission_accounts
            .first()
            .map(TxSigner::address)
            .unwrap_or_else(|| config.account.address()),
        DeploymentMode::DeployAndDelegate => config.account.address(),
    };
    let tx_nonce = wait_for_pending_txs(provider, tx_sender).await?;
    let signed_auth = if mode.includes_delegation() {
        let chain_id = provider
            .get_chain_id()
            .await
            .context("reading chain id for EIP-7702 authorization")?;
        Some(sign_authorization(config, chain_id, delegate, tx_nonce + 1).await?)
    } else {
        None
    };
    let input = CREATE2_SALT
        .iter()
        .chain(init_code)
        .copied()
        .collect::<Bytes>();

    tracing::info!(
        delegate = ?delegate,
        approved_callers = ?approved_callers,
        tx_sender = ?tx_sender,
        tx_nonce,
        mode = ?mode,
        "deploying Solver7702Delegate with CREATE2"
    );
    let mut tx = TransactionRequest::default()
        .from(tx_sender)
        .to(CREATE2_DEPLOYER)
        .nonce(tx_nonce)
        .input(input.into());
    if let Some(signed_auth) = signed_auth {
        tx = tx.with_authorization_list(vec![signed_auth]);
    }

    let pending = provider
        .send_transaction(tx)
        .await
        .context("sending Solver7702Delegate CREATE2 deployment tx")?;
    let receipt = pending
        .get_receipt()
        .await
        .context("waiting for Solver7702Delegate CREATE2 deployment receipt")?;
    receipt
        .ensure_success()
        .context("Solver7702Delegate CREATE2 deployment tx reverted")?;

    let code = provider
        .get_code_at(delegate)
        .await
        .context("reading Solver7702Delegate code after deployment")?;
    anyhow::ensure!(
        !code.is_empty(),
        "Solver7702Delegate deployment tx {:?} did not create code at {:?}",
        receipt.transaction_hash,
        delegate,
    );
    if mode.includes_delegation() {
        let solver_code = provider
            .get_code_at(tx_sender)
            .await
            .context("reading solver EOA code after combined deployment and delegation")?;
        anyhow::ensure!(
            is_delegated_to(&solver_code, delegate),
            "Solver7702Delegate deployment tx {:?} did not delegate solver EOA {:?} to {:?}. \
             Expected auth_nonce={} (solver_nonce={} + 1). Check that no pending txs changed the \
             nonce between query and submission.",
            receipt.transaction_hash,
            tx_sender,
            delegate,
            tx_nonce + 1,
            tx_nonce,
        );
    }
    tracing::info!(
        tx_hash = ?receipt.transaction_hash,
        block = ?receipt.block_number,
        delegate = ?delegate,
        mode = ?mode,
        "Solver7702Delegate CREATE2 deployment confirmed"
    );

    Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DelegationStatus {
    /// No code (`eth_getCode` returns empty); undelegated EOA.
    Empty,
    /// EIP-7702 delegation: code is [`DELEGATION_PREFIX`] followed by this
    /// implementation address.
    DelegatedTo(Address),
    /// Non-empty code that is not an EIP-7702 delegation prefix.
    OtherCode,
}

impl DelegationStatus {
    fn from_code(code: &[u8]) -> Self {
        if code.is_empty() {
            Self::Empty
        } else if code.len() == DELEGATION_CODE_LEN && code.starts_with(&DELEGATION_PREFIX) {
            Self::DelegatedTo(Address::from_slice(&code[DELEGATION_PREFIX.len()..]))
        } else {
            Self::OtherCode
        }
    }
}

/// Check whether the account's code is an EIP-7702 delegation to
/// `expected_delegate`.
fn is_delegated_to(code: &[u8], expected_delegate: Address) -> bool {
    matches!(DelegationStatus::from_code(code), DelegationStatus::DelegatedTo(delegate) if delegate == expected_delegate)
}

/// Set up EIP-7702 delegation with a zero-address authorization transaction.
#[instrument(skip_all)]
async fn setup_delegation(
    config: &Config,
    delegate: Address,
    eth: &Ethereum,
) -> anyhow::Result<()> {
    let provider = &eth.web3().provider;
    let chain_id = provider
        .get_chain_id()
        .await
        .context("reading chain id for EIP-7702 authorization")?;
    let solver_address: Address = config.account.address();

    // Wait for any pending solver txs to clear (e.g. in-flight settlements
    // from a pre-7702 deployment). Submitting at the same nonce would replace
    // the pending tx, silently dropping a valid settlement.
    let solver_nonce = wait_for_pending_txs(provider, solver_address).await?;

    tracing::info!(
        ?delegate,
        solver_nonce,
        auth_nonce = solver_nonce + 1,
        "setting up EIP-7702 solver delegation"
    );

    // The auth nonce must be solver_nonce + 1: in EIP-7702 the sender's nonce
    // is incremented before the authorization list is processed. Since the
    // solver is both sender and authority, the nonce will already be
    // solver_nonce + 1 by the time the auth is checked.
    let signed_auth = sign_authorization(config, chain_id, delegate, solver_nonce + 1).await?;

    // This path is used when the CREATE2 delegate already exists. The tx only
    // carries the auth, so use an inert zero-value call.
    let tx = TransactionRequest::default()
        .from(solver_address)
        .to(Address::ZERO)
        .value(U256::ZERO)
        .nonce(solver_nonce)
        .with_authorization_list(vec![signed_auth]);

    let pending = provider
        .send_transaction(tx)
        .await
        .context("sending EIP-7702 delegation tx")?;
    let receipt = pending
        .get_receipt()
        .await
        .context("waiting for EIP-7702 delegation receipt")?;
    receipt
        .ensure_success()
        .context("EIP-7702 delegation tx reverted")?;
    tracing::info!(
        tx_hash = ?receipt.transaction_hash,
        block = ?receipt.block_number,
        "EIP-7702 delegation tx confirmed"
    );

    // Verify the delegation was actually applied (EIP-7702 silently skips
    // authorizations with mismatched nonces).
    let code = provider
        .get_code_at(solver_address)
        .await
        .context("reading solver EOA code after EIP-7702 delegation tx")?;
    if !is_delegated_to(&code, delegate) {
        anyhow::bail!(
            "EIP-7702 delegation not applied after tx {:?}. Expected auth_nonce={} \
             (solver_nonce={} + 1). Check that no pending txs changed the nonce between query and \
             submission.",
            receipt.transaction_hash,
            solver_nonce + 1,
            solver_nonce,
        );
    }

    Ok(())
}

async fn sign_authorization(
    config: &Config,
    chain_id: u64,
    delegate: Address,
    auth_nonce: u64,
) -> anyhow::Result<SignedAuthorization> {
    let auth = Authorization {
        chain_id: U256::from(chain_id),
        address: delegate,
        nonce: auth_nonce,
    };
    let sig = config
        .account
        .sign_hash(&auth.signature_hash())
        .await
        .context("failed to sign EIP-7702 authorization")?;

    Ok(auth.into_signed(sig))
}

/// Wait until the solver has no pending transactions in the mempool.
/// Returns the confirmed nonce (safe to use for the next tx).
#[instrument(skip_all)]
async fn wait_for_pending_txs(provider: &impl Provider, address: Address) -> anyhow::Result<u64> {
    const POLL_INTERVAL: Duration = Duration::from_secs(3);
    const MAX_WAIT: Duration = Duration::from_secs(90);

    let deadline = tokio::time::Instant::now() + MAX_WAIT;
    loop {
        // Startup can happen while transactions from the previous driver process
        // are still pending. Reusing that nonce would replace them.
        // only counts txs in mined blocks
        let latest = provider
            .get_transaction_count(address)
            .await
            .context("reading latest solver nonce before EIP-7702 setup")?;
        // also count txs in the mempool
        let pending = provider
            .get_transaction_count(address)
            .pending()
            .await
            .context("reading pending solver nonce before EIP-7702 setup")?;
        if pending <= latest {
            return Ok(latest);
        }
        if tokio::time::Instant::now() > deadline {
            anyhow::bail!(
                "timed out waiting for {} pending solver txs to clear (latest nonce: {latest}, \
                 pending nonce: {pending})",
                pending - latest,
            );
        }
        tracing::info!(
            latest_nonce = latest,
            pending_nonce = pending,
            pending_txs = pending - latest,
            "waiting for pending solver txs to clear before delegation setup"
        );
        tokio::time::sleep(POLL_INTERVAL).await;
    }
}

#[cfg(test)]
mod tests {
    use {super::*, alloy::primitives::address};

    const CALLER_A: Address = address!("0000000000000000000000000000000000000001");
    const CALLER_B: Address = address!("0000000000000000000000000000000000000002");
    const CALLER_C: Address = address!("0000000000000000000000000000000000000003");
    const CALLER_D: Address = address!("0000000000000000000000000000000000000004");
    const CALLER_E: Address = address!("0000000000000000000000000000000000000005");
    const CALLER_F: Address = address!("0000000000000000000000000000000000000006");

    #[test]
    fn delegate_target_is_stable_and_caller_sensitive() {
        let (first, _, _) = delegate_deployment(&[CALLER_A, CALLER_B]).unwrap();
        let (same, _, _) = delegate_deployment(&[CALLER_A, CALLER_B]).unwrap();
        let (reordered, _, _) = delegate_deployment(&[CALLER_B, CALLER_A]).unwrap();

        assert_eq!(first, same);
        assert_ne!(first, reordered);
    }

    #[test]
    fn pads_approved_callers_to_contract_capacity() {
        let (_, approved_callers, _) = delegate_deployment(&[CALLER_A, CALLER_B]).unwrap();

        assert_eq!(
            approved_callers,
            [
                CALLER_A,
                CALLER_B,
                Address::ZERO,
                Address::ZERO,
                Address::ZERO
            ]
        );
    }

    #[test]
    fn rejects_more_callers_than_the_delegate_supports() {
        let err =
            delegate_deployment(&[CALLER_A, CALLER_B, CALLER_C, CALLER_D, CALLER_E, CALLER_F])
                .unwrap_err();

        assert!(err.to_string().contains("at most 5"));
    }

    #[test]
    fn rejects_zero_submission_account() {
        let err = delegate_deployment(&[CALLER_A, Address::ZERO]).unwrap_err();

        assert!(err.to_string().contains("zero address"));
    }

    #[test]
    fn rejects_duplicate_submission_accounts() {
        let err = delegate_deployment(&[CALLER_A, CALLER_A]).unwrap_err();

        assert!(err.to_string().contains("must be unique"));
    }

    #[test]
    fn detects_eip7702_delegation_target() {
        let delegate = address!("0000000000000000000000000000000000000007");
        let other = address!("0000000000000000000000000000000000000008");
        let mut code = Vec::from(DELEGATION_PREFIX);
        code.extend_from_slice(delegate.as_slice());

        assert_eq!(DelegationStatus::from_code(&[]), DelegationStatus::Empty);
        assert_eq!(
            DelegationStatus::from_code(&[0x60, 0x00]),
            DelegationStatus::OtherCode
        );
        assert!(is_delegated_to(&code, delegate));
        assert!(!is_delegated_to(&code, other));
    }
}
