//! Pod network account recovery for locked accounts.
//! See: https://docs.v2.pod.network/guides-references/guides/recover-locked-account

use {
    alloy::{providers::Provider, sol},
    pod_sdk::{Address, alloy_primitives::B256, provider::PodProvider},
    thiserror::Error,
};

const RECOVERY_PRECOMPILE: &str = "0x50d0000000000000000000000000000000000003";

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

#[derive(Debug, Error)]
pub enum RecoveryError {
    #[error("get target: {0}")]
    GetTarget(String),
    #[error("send: {0}")]
    Send(String),
    #[error("tx failed: {0}")]
    TxFailed(String),
    #[error("not locked")]
    NotLocked,
}

#[tracing::instrument(skip_all, fields(%account))]
pub async fn recover_locked_account(
    provider: &PodProvider,
    account: Address,
) -> Result<bool, RecoveryError> {
    let target = match get_recovery_target(provider, account).await {
        Ok(t) => t,
        Err(RecoveryError::NotLocked) => return Ok(false),
        Err(e) => return Err(e),
    };

    tracing::info!(tx_hash = %target.tx_hash, nonce = target.nonce, "recovering");

    let precompile: Address = RECOVERY_PRECOMPILE.parse().unwrap();
    let receipt = Recovery::new(precompile, provider)
        .recover(target.tx_hash, target.nonce)
        .send()
        .await
        .map_err(|e| RecoveryError::Send(e.to_string()))?
        .get_receipt()
        .await
        .map_err(|e| RecoveryError::TxFailed(e.to_string()))?;

    if !receipt.status() {
        return Err(RecoveryError::TxFailed("reverted".into()));
    }

    tracing::info!("recovered");
    Ok(true)
}

async fn get_recovery_target(
    provider: &PodProvider,
    account: Address,
) -> Result<RecoveryTarget, RecoveryError> {
    let result: serde_json::Value = provider
        .raw_request("pod_getRecoveryTargetTx".into(), vec![account])
        .await
        .map_err(|e| {
            let s = e.to_string();
            if s.contains("not locked") || s.contains("no recovery") {
                RecoveryError::NotLocked
            } else {
                RecoveryError::GetTarget(s)
            }
        })?;
    serde_json::from_value(result).map_err(|e| RecoveryError::GetTarget(e.to_string()))
}

pub fn is_account_locked_error(error: &str) -> bool {
    error.contains("Another transaction") && error.contains("is still pending")
}
