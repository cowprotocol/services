#[macro_export]
macro_rules! dummy_contract {
    ($contract:ty, $addr:expr) => {
        <$contract>::at(&$crate::web3::dummy(), $addr.into())
    };
}

#[macro_export]
macro_rules! bytecode {
    ($contract:ty) => {
        <$contract>::raw_contract().bytecode.to_bytes().unwrap()
    };
}

#[macro_export]
macro_rules! deployed_bytecode {
    ($contract:ty) => {
        <$contract>::raw_contract()
            .deployed_bytecode
            .to_bytes()
            .unwrap()
    };
}

#[macro_export]
macro_rules! deployment_block {
    ($contract:ident) => {
        match $contract.deployment_information() {
            Some(ethcontract::common::DeploymentInformation::TransactionHash(_)) => {
                panic!("no block number in deployment info")
            }
            Some(ethcontract::common::DeploymentInformation::BlockNumber(block)) => Some(block),
            None => None,
        }
    };
}
