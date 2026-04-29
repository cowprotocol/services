use {alloy_contract::Error as ContractError, alloy_transport::RpcError};

/// Bubbles up node errors, ignoring all other errors.
pub fn ignore_non_node_error<T>(result: Result<T, ContractError>) -> anyhow::Result<Option<T>> {
    match result {
        Ok(result) => Ok(Some(result)),
        Err(err) if err.is_node_error() => Err(err.into()),
        Err(_) => Ok(None),
    }
}

pub trait ContractErrorExt {
    /// Returns whether a given error is a contract error, this is considered to
    /// be all errors except the transport error where there is no revert data.
    fn is_contract_error(&self) -> bool;

    /// Returns whether a given error is a node error.
    fn is_node_error(&self) -> bool;

    /// Contract-level rejection of the call: an explicit revert (including
    /// empty-data reverts from missing selectors, which [`is_contract_error`]
    /// misses) or a `0x` response. Transport failures and caller-side bugs
    /// return `false` so they keep bubbling up for retry.
    fn is_contract_revert(&self) -> bool;
}

impl ContractErrorExt for ContractError {
    fn is_contract_error(&self) -> bool {
        !self.is_node_error()
    }

    fn is_node_error(&self) -> bool {
        // This mapping is "ported" from ethcontract's error hierarchy, although in it
        // contract errors are better defined. Essentially, everything that isn't web3
        // related in ethcontract gets classified as a contract error, however, in alloy
        // some contract errors are "hidden" inside transport errors, as such we need to
        // check if transport errors have revert data to rule out contract errors.
        //
        // NOTE: using alloy's decoding functions doesn't work here because it requires
        // the revert data to *not* be empty, otherwise it will return `None` as if a
        // revert wasn't present or wasn't able to be decoded, though the usage of
        // `Option` erases the existing nuances of the failure
        match self {
            // When the revert data is empty (e.g. you perform a call to a missing function)
            // alloy's decode breaks, because even though there is revert data, it is empty
            // so alloy's decode method fails leading your (the caller) to think there wasn't one
            // Thus, to properly check for any kind of revert, just look at the revert data
            ContractError::TransportError(RpcError::ErrorResp(err)) => {
                // Due to the mismatch between error APIs and best-effort approximation this log
                // line is left here as a debugging tool in case we start having RPC issues
                let no_revert_data = err.as_revert_data().is_none();
                tracing::debug!(?err, %no_revert_data, "transport rpc error");
                no_revert_data
            }
            ContractError::TransportError(_) => true,
            _ => false,
        }
    }

    fn is_contract_revert(&self) -> bool {
        match self {
            // Revert data, geth code 3, a "revert" message, or specifically
            // the `INVALID`/0xFE opcode that older Solidity emits when no
            // selector matches — all are deterministic contract-level
            // rejections. Other EVM halts (e.g. `InvalidJump`) can stem from
            // bad input rather than a contract-level rejection, so we don't
            // lump them in here. Other ErrorResps (rate limits, bad params)
            // are transport and must retry.
            ContractError::TransportError(RpcError::ErrorResp(err)) => {
                let message = err.message.to_lowercase();
                err.as_revert_data().is_some()
                    // https://github.com/ethereum/go-ethereum/blob/8e2107dc39dc9dab132150ec915e7ac299f9eb48/internal/ethapi/errors.go#L42-L46
                    // https://github.com/alloy-rs/alloy/blob/b6753088241a50730c092bdba7036f52887c4c57/crates/rpc-types-eth/src/error.rs#L32
                    || err.code == 3
                    || message.contains("revert")
                    // anvil/revm surfaces halt reasons as
                    // `EVM error <HaltReason>`. Match only the `INVALID`
                    // (0xFE) opcode here — older Solidity (e.g. Bancor BNT)
                    // emits it on a missing selector, which is a contract-
                    // level rejection. Other halts are intentionally excluded.
                    || message.contains("invalidfeopcode")
            }
            ContractError::ZeroData(..)
            | ContractError::UnknownFunction(..)
            | ContractError::UnknownSelector(..) => true,
            _ => false,
        }
    }
}

/// Create an arbitrary alloy error that will convert into a "contract" error.
/// Useful for testing.
#[cfg(any(test, feature = "test-util"))]
pub fn testing_alloy_contract_error() -> alloy_contract::Error {
    alloy_contract::Error::NotADeploymentTransaction
}

/// Create an arbitrary alloy error that will convert into a "node" error.
/// Useful for testing.
#[cfg(any(test, feature = "test-util"))]
pub fn testing_alloy_node_error() -> alloy_contract::Error {
    alloy_contract::Error::TransportError(alloy_transport::TransportError::ErrorResp(
        alloy_json_rpc::ErrorPayload::internal_error(),
    ))
}

#[cfg(test)]
mod tests {
    use {
        crate::alloy::errors::{
            ContractErrorExt,
            testing_alloy_contract_error,
            testing_alloy_node_error,
        },
        alloy_contract::Error as ContractError,
        alloy_json_rpc::ErrorPayload,
        alloy_transport::TransportError,
    };

    fn error_resp(code: i64, message: &'static str) -> ContractError {
        ContractError::TransportError(TransportError::ErrorResp(ErrorPayload {
            code,
            message: message.into(),
            data: None,
        }))
    }

    #[test]
    fn test_contract_error() {
        assert!(testing_alloy_contract_error().is_contract_error());
        assert!(!testing_alloy_node_error().is_contract_error());
    }

    #[test]
    fn test_node_error() {
        assert!(!testing_alloy_contract_error().is_node_error());
        assert!(testing_alloy_node_error().is_node_error());
    }

    #[test]
    fn contract_revert_accepts_empty_data_reverts() {
        // Geth-family "execution reverted" with no data — the USDC case.
        assert!(error_resp(3, "execution reverted").is_contract_revert());
        // Non-standard code but unmistakable message.
        assert!(error_resp(-32000, "execution reverted at pc=...").is_contract_revert());
        assert!(error_resp(-32000, "VM Exception: revert").is_contract_revert());
        // Case-insensitive: capitalized node messages still match.
        assert!(error_resp(-32000, "Execution Reverted").is_contract_revert());
    }

    #[test]
    fn contract_revert_accepts_invalid_fe_opcode_halt() {
        // anvil surfaces the INVALID (0xFE) opcode as `EVM error InvalidFEOpcode`
        // — older Solidity (e.g. Bancor BNT) emits it on a missing selector.
        assert!(error_resp(-32603, "EVM error InvalidFEOpcode").is_contract_revert());
        // Case-insensitive match.
        assert!(error_resp(-32603, "evm error invalidfeopcode").is_contract_revert());
    }

    #[test]
    fn contract_revert_rejects_other_evm_halts() {
        // Other halts (e.g. `InvalidJump`) can be triggered by bad input
        // rather than a contract-level rejection, so they must keep bubbling
        // up rather than being classified as reverts.
        assert!(!error_resp(-32603, "EVM error InvalidJump").is_contract_revert());
        assert!(!error_resp(-32603, "EVM error OpcodeNotFound").is_contract_revert());
        assert!(!error_resp(-32603, "EVM error StackUnderflow").is_contract_revert());
    }

    #[test]
    fn contract_revert_rejects_transport_failures() {
        // Generic internal error without revert context — likely transport.
        assert!(!testing_alloy_node_error().is_contract_revert());
        // Rate-limit-like codes without a revert message.
        assert!(!error_resp(429, "too many requests").is_contract_revert());
        assert!(!error_resp(-32005, "daily request count exceeded").is_contract_revert());
    }

    #[test]
    fn contract_revert_rejects_caller_usage_bugs() {
        // `NotADeploymentTransaction` and siblings are local-usage errors,
        // not contract behaviour — must not be classified as reverts.
        assert!(!testing_alloy_contract_error().is_contract_revert());
    }
}
