//! Pod network account recovery for locked accounts.
//! See: https://docs.v2.pod.network/guides-references/guides/recover-locked-account

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

pub fn is_account_locked_error(error: &str) -> bool {
    error.contains("Another transaction") && error.contains("is still pending")
}
