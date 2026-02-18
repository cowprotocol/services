use {
    super::{Config, Solver},
    crate::infra::blockchain::Ethereum,
    alloy::{
        eips::eip7702::Authorization,
        network::{TransactionBuilder7702, TxSigner},
        primitives::{Address, U256},
        providers::Provider,
        rpc::types::TransactionRequest,
        sol_types::SolCall,
    },
    contracts::alloy::CowSettlementForwarder::CowSettlementForwarder,
    futures::future::join_all,
    tracing::instrument,
};

/// EIP-7702 delegation prefix stored as account code.
const DELEGATION_PREFIX: [u8; 3] = [0xef, 0x01, 0x00];

/// Ensure EIP-7702 delegation and caller approval are set up for all solvers
/// with parallel submission accounts. Called once at driver startup.
#[instrument(name = "setup_eip7702", skip_all)]
pub async fn setup(solvers: &[Solver], eth: &Ethereum) -> anyhow::Result<()> {
    for solver in solvers {
        let config = solver.config();
        if config.submission_accounts.is_empty() {
            continue;
        }
        if matches!(config.account, super::Account::Address(_)) {
            tracing::debug!(solver = %config.name, "dry-run mode, skipping EIP-7702 setup");
            continue;
        }
        let forwarder = config.forwarder_contract.ok_or_else(|| {
            anyhow::anyhow!(
                "solver {}: submission_accounts configured but forwarder_contract missing",
                config.name
            )
        })?;

        // Register solver + submission accounts with the main wallet so we can
        // send transactions via the provider during setup.
        let web3 = eth.web3();
        web3.wallet.register_signer(config.account.clone());
        for acc in &config.submission_accounts {
            web3.wallet.register_signer(acc.clone());
        }

        setup_solver(config, forwarder, eth).await?;
    }
    Ok(())
}

#[instrument(skip_all)]
async fn setup_solver(config: &Config, forwarder: Address, eth: &Ethereum) -> anyhow::Result<()> {
    let solver_address: Address = config.account.address();
    let provider = &eth.web3().provider;

    // Check delegation status.
    let code = provider.get_code_at(solver_address).await?;
    let needs_delegation = !is_delegated_to(&code, forwarder);

    // Only check caller approvals if delegation is already active (otherwise
    // the solver EOA has no code and eth_call would fail).
    let submission_addresses: Vec<Address> = config
        .submission_accounts
        .iter()
        .map(TxSigner::address)
        .collect();

    if needs_delegation {
        setup_delegation_and_approve(config, forwarder, &submission_addresses, eth).await?;
    } else {
        let unapproved =
            check_unapproved_callers(eth, solver_address, &submission_addresses).await?;
        if !unapproved.is_empty() {
            approve_submitters(config, &unapproved, eth).await?;
        } else {
            tracing::debug!("delegation and caller approvals already configured");
        }
    }

    Ok(())
}

/// Check whether the account's code is an EIP-7702 delegation to
/// `expected_forwarder`.
#[instrument(skip_all)]
fn is_delegated_to(code: &[u8], expected_forwarder: Address) -> bool {
    // EIP-7702 delegation designator: 0xef0100 || 20-byte address
    code.len() == 23 && code.starts_with(&DELEGATION_PREFIX) && code[3..] == expected_forwarder.0.0
}

/// Check which submission accounts are already approved callers on the
/// solver's delegated forwarder. Uses `join_all` which auto-batches through
/// ethrpc's batching layer.
#[instrument(skip_all)]
async fn check_unapproved_callers(
    eth: &Ethereum,
    solver: Address,
    callers: &[Address],
) -> anyhow::Result<Vec<Address>> {
    let provider = &eth.web3().provider;

    let results: Vec<bool> = join_all(callers.iter().map(|caller| {
        let caller = *caller;
        async move {
            let data = CowSettlementForwarder::isApprovedCallerCall(caller).abi_encode();
            let tx = TransactionRequest::default().to(solver).input(data.into());
            let output = provider.call(tx).await?;
            Ok(CowSettlementForwarder::isApprovedCallerCall::abi_decode_returns(&output)?)
        }
    }))
    .await
    .into_iter()
    .collect::<anyhow::Result<_>>()?;

    Ok(callers
        .iter()
        .zip(results)
        .filter(|(_, approved)| !approved)
        .map(|(addr, _)| *addr)
        .collect())
}

/// Set up EIP-7702 delegation and approve callers in a single transaction.
/// The solver signs the authorization and self-calls `setApprovedCallers`.
#[instrument(skip_all)]
async fn setup_delegation_and_approve(
    config: &Config,
    forwarder: Address,
    unapproved: &[Address],
    eth: &Ethereum,
) -> anyhow::Result<()> {
    tracing::info!(
        ?forwarder,
        unapproved_callers = unapproved.len(),
        "setting up EIP-7702 delegation"
    );
    let provider = &eth.web3().provider;
    let chain_id = provider.get_chain_id().await?;
    let solver_address: Address = config.account.address();
    let solver_nonce = provider.get_transaction_count(solver_address).await?;

    // Sign the EIP-7702 authorization (solver delegates to forwarder).
    let auth = Authorization {
        chain_id: U256::from(chain_id),
        address: forwarder,
        nonce: solver_nonce,
    };
    let sig = config
        .account
        .sign_hash(&auth.signature_hash())
        .await
        .map_err(|e| anyhow::anyhow!("failed to sign EIP-7702 authorization: {e}"))?;
    let signed_auth = auth.into_signed(sig);

    // Solver self-calls with auth list. After the authorization is applied the
    // forwarder code runs, and `msg.sender == address(this)` passes the auth
    // check in `setApprovedCallers`.
    let mut tx = TransactionRequest::default()
        .from(solver_address)
        .to(solver_address)
        .with_authorization_list(vec![signed_auth]);

    if !unapproved.is_empty() {
        let calldata = CowSettlementForwarder::setApprovedCallersCall {
            callers: unapproved.to_vec(),
            approved: true,
        }
        .abi_encode();
        tx = tx.input(calldata.into());
    } else {
        tx = tx.value(U256::ZERO);
    }

    let pending = provider.send_transaction(tx).await?;
    let receipt = pending.get_receipt().await?;
    tracing::info!(
        tx_hash = ?receipt.transaction_hash,
        block = ?receipt.block_number,
        "EIP-7702 delegation tx confirmed"
    );

    Ok(())
}

/// Approve callers via a solver self-call (delegation already active).
#[instrument(skip_all)]
async fn approve_submitters(
    config: &Config,
    unapproved: &[Address],
    eth: &Ethereum,
) -> anyhow::Result<()> {
    tracing::info!(
        unapproved_callers = unapproved.len(),
        "approving new submission callers"
    );
    let provider = &eth.web3().provider;
    let solver_address: Address = config.account.address();

    let calldata = CowSettlementForwarder::setApprovedCallersCall {
        callers: unapproved.to_vec(),
        approved: true,
    }
    .abi_encode();

    let tx = TransactionRequest::default()
        .from(solver_address)
        .to(solver_address)
        .input(calldata.into());

    let pending = provider.send_transaction(tx).await?;
    let receipt = pending.get_receipt().await?;
    tracing::info!(
        tx_hash = ?receipt.transaction_hash,
        block = ?receipt.block_number,
        "setApprovedCallers tx confirmed"
    );

    Ok(())
}
