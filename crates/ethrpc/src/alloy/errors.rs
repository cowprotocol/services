use alloy::{contract::Error as ContractError, sol_types::GenericContractError};

/// Bubbles up node errors, ignoring all other errors.
// This function can be made to return only the RPC error that wasn't decode
pub fn handle_alloy_contract_error<T>(
    result: Result<T, ContractError>,
) -> anyhow::Result<Option<T>> {
    match result {
        Ok(result) => Ok(Some(result)),
        // Alloy "hides" the contract execution errors under the transport error
        Err(err @ alloy::contract::Error::TransportError(_)) => {
            // So we need to try to decode the error as a generic contract error,
            // we return in case it isn't a contract error
            match err.try_decode_into_interface_error::<GenericContractError>() {
                Ok(_) => Ok(None), // contract error
                Err(err) => Err(err)?,
            }
        }
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
