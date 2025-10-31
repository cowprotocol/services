use alloy::{contract::Error as ContractError, transports::RpcError};

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
}

/// Create an arbitrary alloy error that will convert into a "contract" error.
/// Useful for testing.
#[cfg(any(test, feature = "test-util"))]
pub fn testing_alloy_contract_error() -> alloy::contract::Error {
    alloy::contract::Error::NotADeploymentTransaction
}

/// Create an arbitrary alloy error that will convert into a "node" error.
/// Useful for testing.
#[cfg(any(test, feature = "test-util"))]
pub fn testing_alloy_node_error() -> alloy::contract::Error {
    alloy::contract::Error::TransportError(alloy::transports::TransportError::ErrorResp(
        alloy::rpc::json_rpc::ErrorPayload::internal_error(),
    ))
}

#[cfg(test)]
mod tests {
    use crate::alloy::errors::{
        ContractErrorExt,
        testing_alloy_contract_error,
        testing_alloy_node_error,
    };

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
}
