use alloy::{contract::Error as ContractError, rpc::json_rpc::ErrorPayload, transports::RpcError};

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
    /// be all errors except the transport error.
    fn is_contract_error(&self) -> bool;

    /// Returns whether a given error is specifically a transport error.
    ///
    /// alloy has two layers for transport nodes, this checks for the inner
    /// most; for a more general function use
    /// [`is_node_error`](ContractErrorExt::is_node_error).
    fn is_transport_error(&self) -> bool;

    /// Returns whether a given error is a node error.
    ///
    /// alloy has two layers for transport nodes, this checks for the outer
    /// most; for transport-only errors use
    /// [`is_transport_error`](ContractErrorExt::is_transport_error).
    fn is_node_error(&self) -> bool;
}

impl ContractErrorExt for ContractError {
    fn is_contract_error(&self) -> bool {
        !self.is_node_error()
    }

    fn is_transport_error(&self) -> bool {
        matches!(self, ContractError::TransportError(RpcError::Transport(_)))
    }

    fn is_node_error(&self) -> bool {
        matches!(
            self,
            ContractError::TransportError(RpcError::ErrorResp(_))
                | ContractError::TransportError(RpcError::Transport(_))
        )
    }
}

/// Create an arbitrary alloy error that will convert into a "contract" error.
/// Useful for testing.
#[cfg(any(test, feature = "test-util"))]
pub fn testing_alloy_contract_error() -> alloy::contract::Error {
    alloy::contract::Error::NotADeploymentTransaction
}

/// Create an arbitrary alloy error that will convert into a "transport" error.
/// Useful for testing.
#[cfg(any(test, feature = "test-util"))]
pub fn testing_alloy_transport_error() -> alloy::contract::Error {
    alloy::contract::Error::TransportError(alloy::transports::TransportError::Transport(
        alloy::transports::TransportErrorKind::BackendGone,
    ))
}

/// Create an arbitrary alloy error that will convert into a "node" error.
/// Useful for testing.
#[cfg(any(test, feature = "test-util"))]
pub fn testing_alloy_node_error() -> alloy::contract::Error {
    alloy::contract::Error::TransportError(alloy::transports::TransportError::ErrorResp(
        ErrorPayload::internal_error(),
    ))
}

#[cfg(test)]
mod tests {
    use crate::alloy::errors::{
        ContractErrorExt,
        testing_alloy_contract_error,
        testing_alloy_node_error,
        testing_alloy_transport_error,
    };

    #[test]
    fn test_contract_error() {
        assert!(testing_alloy_contract_error().is_contract_error());
        assert!(!testing_alloy_transport_error().is_contract_error());
        assert!(!testing_alloy_node_error().is_transport_error());
    }

    #[test]
    fn test_transport_error() {
        assert!(!testing_alloy_contract_error().is_transport_error());
        assert!(testing_alloy_transport_error().is_transport_error());
        assert!(!testing_alloy_node_error().is_transport_error());
    }

    #[test]
    fn test_node_error() {
        assert!(!testing_alloy_contract_error().is_node_error());
        assert!(testing_alloy_transport_error().is_node_error());
        assert!(testing_alloy_node_error().is_node_error());
    }
}
