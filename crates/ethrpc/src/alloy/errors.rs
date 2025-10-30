use alloy::contract::Error as ContractError;

/// Bubbles up node errors, ignoring all other errors.
pub fn ignore_non_transport_error<T>(
    result: Result<T, ContractError>,
) -> anyhow::Result<Option<T>> {
    match result {
        Ok(result) => Ok(Some(result)),
        Err(err) if err.is_contract_err() => Err(err.into()),
        Err(_) => Ok(None),
    }
}

pub trait ContractErrorExt {
    /// Returns if a given error is a contract error, this is considered to be
    /// all errors except the transport error.
    fn is_contract_err(&self) -> bool;

    /// Returns if a given error is a transport error.
    fn is_transport_error(&self) -> bool;
}

impl ContractErrorExt for ContractError {
    fn is_contract_err(&self) -> bool {
        !self.is_transport_error()
    }

    fn is_transport_error(&self) -> bool {
        if let ContractError::TransportError(rpc_error) = self {
            rpc_error.is_transport_error()
        } else {
            false
        }
    }
}
