use ethcontract::errors::{ExecutionError, MethodError};

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum EthcontractErrorType {
    // The error stems from communicating with the node.
    Node,
    // Communication was successful but the contract on chain errored.
    Contract,
}

impl EthcontractErrorType {
    pub fn classify(err: &MethodError) -> Self {
        match &err.inner {
            ExecutionError::Web3(_) => Self::Node,
            _ => Self::Contract,
        }
    }

    /// Returns true if the specified error is a contract error.
    ///
    /// This is short hand for calling `classify` and checking it returns a `Contract` variant.
    pub fn is_contract_err(err: &MethodError) -> bool {
        matches!(Self::classify(err), Self::Contract)
    }
}

// Create an arbitrary error. Useful for testing.
pub fn testing_node_error() -> MethodError {
    MethodError {
        signature: String::new(),
        inner: ExecutionError::Web3(web3::Error::Internal),
    }
}

// Create an arbitrary error. Useful for testing.
pub fn testing_contract_error() -> MethodError {
    MethodError {
        signature: String::new(),
        inner: ExecutionError::InvalidOpcode,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn node_error() {
        assert_eq!(
            EthcontractErrorType::classify(&testing_node_error()),
            EthcontractErrorType::Node
        );
    }

    #[test]
    fn contract_error() {
        assert_eq!(
            EthcontractErrorType::classify(&testing_contract_error()),
            EthcontractErrorType::Contract
        );
    }
}
