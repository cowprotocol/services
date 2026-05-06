//! Pod network account recovery for locked accounts.
//!
//! Pod accepts at most one in-flight transaction per submitter address.
//! Submitting a second transaction before the first is mined causes pod to
//! mark the account as *locked*; subsequent submissions fail until the
//! account is recovered via `pod_getRecoveryTargetTx` + the recovery
//! precompile. See:
//! <https://docs.v2.pod.network/guides-references/guides/recover-locked-account>
//!
//! TODO(production-pod): the fire-and-forget submit + best-effort recovery
//! shape is shadow-mode-only. Before pod runs the auction for real this must
//! be reworked: queueing semantics for concurrent submissions per solver
//! EOA, retry/backoff policy, observability + alerting on locked-account
//! streaks, and propagation of arbitration results for cross-validation
//! against the autopilot's ranking.

use {
    alloy::{primitives::address, providers::Provider, sol},
    anyhow::{Context, anyhow},
    pod_sdk::{Address, alloy_primitives::B256, provider::PodProvider},
};

const RECOVERY_PRECOMPILE: Address = address!("50d0000000000000000000000000000000000003");

sol! {
    #[sol(rpc)]
    contract Recovery {
        function recover(bytes32 txHash, uint64 nonce) public;
    }
}

#[derive(Debug, Clone, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct RecoveryTarget {
    tx_hash: B256,
    nonce: u64,
}

/// Attempts to recover a locked pod account. Returns `Ok(true)` on successful
/// recovery, `Ok(false)` if the account is not locked, and `Err` on RPC or
/// transaction failure.
#[tracing::instrument(skip_all, fields(%account))]
pub async fn recover_locked_account(
    provider: &PodProvider,
    account: Address,
) -> anyhow::Result<bool> {
    let Some(target) = get_recovery_target(provider, account).await? else {
        return Ok(false);
    };

    tracing::info!(tx_hash = %target.tx_hash, nonce = target.nonce, "recovering");

    let receipt = Recovery::new(RECOVERY_PRECOMPILE, provider)
        .recover(target.tx_hash, target.nonce)
        .send()
        .await
        .context("send recovery tx")?
        .get_receipt()
        .await
        .context("recovery tx receipt")?;

    if !receipt.status() {
        return Err(anyhow!("recovery tx reverted"));
    }

    tracing::info!("recovered");
    Ok(true)
}

async fn get_recovery_target(
    provider: &PodProvider,
    account: Address,
) -> anyhow::Result<Option<RecoveryTarget>> {
    let result = provider
        .raw_request::<_, serde_json::Value>("pod_getRecoveryTargetTx".into(), vec![account])
        .await;

    match result {
        Ok(value) => Ok(Some(
            serde_json::from_value(value).context("decode target")?,
        )),
        Err(e) => {
            let s = e.to_string();
            if s.contains("not locked") || s.contains("no recovery") {
                Ok(None)
            } else {
                Err(anyhow!(s).context("pod_getRecoveryTargetTx"))
            }
        }
    }
}

/// Detects the "account is locked" condition in an error returned by
/// `AuctionClient::submit_bid`.
///
/// Pod limits each submitter to one in-flight transaction; when a second
/// submission arrives before the first is mined, pod returns an RPC error
/// containing the substrings matched below. The matcher is brittle: pod-sdk
/// 0.5.1 returns a stringly RPC error with no typed variant, so the wire
/// format is the only thing to match on. Pinning the SDK version gives a
/// future bump a fair chance to break the test and force re-evaluation.
///
/// On a hit we attempt recovery so the next bid in the auction's hot path
/// can proceed; failing the whole submission would lose the bid for an
/// otherwise-recoverable transient.
pub fn is_account_locked_error(error: &str) -> bool {
    error.contains("Another transaction") && error.contains("is still pending")
}
