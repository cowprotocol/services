use {
    super::{Config, Solver},
    crate::infra::blockchain::Ethereum,
    alloy::{
        eips::eip7702::Authorization,
        network::{TransactionBuilder7702, TxSigner},
        primitives::{Address, B256, Bytes, U256, address},
        providers::Provider,
        rpc::types::TransactionRequest,
        sol_types::SolConstructor,
    },
    anyhow::Context,
    contracts::Solver7702Delegate::Solver7702Delegate,
    hex_literal::hex,
    std::time::Duration,
    tracing::instrument,
};

/// EIP-7702 delegation prefix stored as account code prefix. If you call
/// eth_getCode on a delegated EOA, instead of getting empty bytes (normal EOA),
/// you get 0xef0100<20-byte contract address>.
const DELEGATION_PREFIX: [u8; 3] = [0xef, 0x01, 0x00];
/// The maximum number of approved callers allowed by the Solver7702Delegate
/// ABI.
const MAX_APPROVED_CALLERS: usize = 5;
type ApprovedCallers = [Address; MAX_APPROVED_CALLERS];
// Arachnid's deterministic-deployment-proxy. It is deployed at this same
// address on many EVM chains. Sending 32-byte salt || init code to it deploys
// that init code with CREATE2. We use it to derive and deploy the exact
// Solver7702Delegate address from this proxy address, zero salt, and init code.
const CREATE2_DEPLOYER: Address = address!("4e59b44847b379578588920cA78FbF26c0B4956C");
const CREATE2_DEPLOYER_CODE: &[u8] = &hex!(
    "7fffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffe03601600081602082378035828234f58015156039578182fd5b8082525050506014600cf3"
);
// The ordered caller slots and zero salt are part of the CREATE2 input, so the
// same caller set keeps resolving to the same delegate target.
const CREATE2_SALT: B256 = B256::ZERO;

/// Ensure EIP-7702 delegate deployment and solver delegation are set up for all
/// solvers with parallel submission accounts. Called once at driver startup.
#[instrument(name = "setup_eip7702", skip_all)]
pub async fn setup(solvers: &[Solver], eth: &Ethereum) -> anyhow::Result<()> {
    for solver in solvers {
        let config = solver.config();
        if config.submission_accounts.is_empty() {
            continue;
        }

        anyhow::ensure!(
            !matches!(config.account, super::Account::Address(_)),
            "solver '{}': main account must be a signer to set up EIP-7702 delegation when \
             submission accounts are configured",
            config.name,
        );

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

fn delegate_deployment(callers: &[Address]) -> anyhow::Result<(Address, ApprovedCallers, Bytes)> {
    anyhow::ensure!(
        callers.len() <= MAX_APPROVED_CALLERS,
        "Solver7702Delegate supports at most {MAX_APPROVED_CALLERS} submission accounts"
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

    let target = CREATE2_DEPLOYER.create2_from_code(CREATE2_SALT, &init_code);

    Ok((target, approved_callers, init_code))
}

#[instrument(skip_all, fields(delegate = ?delegate))]
async fn setup_solver(
    config: &Config,
    delegate: Address,
    approved_callers: &ApprovedCallers,
    init_code: &Bytes,
    eth: &Ethereum,
) -> anyhow::Result<()> {
    deploy_delegate_if_missing(config, delegate, approved_callers, init_code, eth).await?;

    let solver_address = config.account.address();
    let code = eth.web3().provider.get_code_at(solver_address).await?;
    if is_delegated_to(&code, delegate) {
        tracing::info!(
            solver = %config.name,
            delegate = ?delegate,
            "solver EOA already delegates to Solver7702Delegate"
        );
        return Ok(());
    }

    setup_delegation(config, delegate, eth).await
}

#[instrument(skip_all, fields(delegate = ?delegate))]
async fn deploy_delegate_if_missing(
    config: &Config,
    delegate: Address,
    approved_callers: &ApprovedCallers,
    init_code: &Bytes,
    eth: &Ethereum,
) -> anyhow::Result<()> {
    let provider = &eth.web3().provider;
    let code = provider.get_code_at(delegate).await?;
    if !code.is_empty() {
        tracing::info!(
            delegate = ?delegate,
            approved_callers = ?approved_callers,
            "reusing existing Solver7702Delegate CREATE2 deployment"
        );
        return Ok(());
    }

    let deployer_code = provider.get_code_at(CREATE2_DEPLOYER).await?;
    anyhow::ensure!(
        deployer_code.as_ref() == CREATE2_DEPLOYER_CODE,
        "CREATE2 deployer {CREATE2_DEPLOYER:?} has unexpected code",
    );

    let tx_sender = config.account.address();
    let tx_nonce = wait_for_pending_txs(provider, tx_sender).await?;
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
        "deploying Solver7702Delegate with CREATE2"
    );
    let pending = provider
        .send_transaction(
            TransactionRequest::default()
                .from(tx_sender)
                .to(CREATE2_DEPLOYER)
                .nonce(tx_nonce)
                .input(input.into()),
        )
        .await?;
    let receipt = pending.get_receipt().await?;

    let code = provider.get_code_at(delegate).await?;
    anyhow::ensure!(
        !code.is_empty(),
        "Solver7702Delegate deployment tx {:?} did not create code at {:?}",
        receipt.transaction_hash,
        delegate,
    );
    tracing::info!(
        tx_hash = ?receipt.transaction_hash,
        block = ?receipt.block_number,
        delegate = ?delegate,
        "Solver7702Delegate CREATE2 deployment confirmed"
    );

    Ok(())
}

/// Check whether the account's code is an EIP-7702 delegation to
/// `expected_delegate`.
fn is_delegated_to(code: &[u8], expected_delegate: Address) -> bool {
    // EIP-7702 delegation designator: 0xef0100 || 20-byte address
    code.len() == 23 && code.starts_with(&DELEGATION_PREFIX) && code[3..] == expected_delegate.0.0
}

/// Set up EIP-7702 delegation with a zero-address authorization transaction.
#[instrument(skip_all)]
async fn setup_delegation(
    config: &Config,
    delegate: Address,
    eth: &Ethereum,
) -> anyhow::Result<()> {
    let provider = &eth.web3().provider;
    let chain_id = provider.get_chain_id().await?;
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
    let auth = Authorization {
        chain_id: U256::from(chain_id),
        address: delegate,
        nonce: solver_nonce + 1,
    };
    let sig = config
        .account
        .sign_hash(&auth.signature_hash())
        .await
        .context("failed to sign EIP-7702 authorization")?;
    let signed_auth = auth.into_signed(sig);

    // Do not combine this with CREATE2 deployment: if execution reverts,
    // EIP-7702 keeps the delegation. This tx only carries the auth, so use an
    // inert zero-value call.
    let tx = TransactionRequest::default()
        .from(solver_address)
        .to(Address::ZERO)
        .value(U256::ZERO)
        .nonce(solver_nonce)
        .with_authorization_list(vec![signed_auth]);

    let pending = provider.send_transaction(tx).await?;
    let receipt = pending.get_receipt().await?;
    tracing::info!(
        tx_hash = ?receipt.transaction_hash,
        block = ?receipt.block_number,
        "EIP-7702 delegation tx confirmed"
    );

    // Verify the delegation was actually applied (EIP-7702 silently skips
    // authorizations with mismatched nonces).
    let code = provider.get_code_at(solver_address).await?;
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
        let latest = provider.get_transaction_count(address).await?;
        // also count txs in the mempool
        let pending = provider.get_transaction_count(address).pending().await?;
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
mod tests;
