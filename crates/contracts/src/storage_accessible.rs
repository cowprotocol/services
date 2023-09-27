//! Module to help encoding `eth_call`s for the `StorageAccessible` contracts.

use {
    crate::support::SimulateCode,
    ethcontract::{
        common::abi,
        contract::MethodBuilder,
        errors::MethodError,
        tokens::Tokenize,
        web3::{
            types::{Bytes, CallRequest},
            Transport,
            Web3,
        },
        H160,
    },
};

/// Encode a call to a `StorageAccessible` `target` to execute `call` with the
/// contract created with `code`
pub fn call(target: H160, code: Bytes, call: Bytes) -> CallRequest {
    // Unfortunately, the `ethcontract` crate does not expose the logic to build
    // creation code for a contract. Luckily, it isn't complicated - you just
    // append the ABI-encoded constructor arguments.
    let args = abi::encode(&[
        target.into_token(),
        ethcontract::Bytes(code.0).into_token(),
        ethcontract::Bytes(call.0).into_token(),
    ]);

    CallRequest {
        data: Some(
            [
                SimulateCode::raw_contract()
                    .bytecode
                    .to_bytes()
                    .unwrap()
                    .0
                    .as_slice(),
                &args,
            ]
            .concat()
            .into(),
        ),
        ..Default::default()
    }
}

/// Simulates the specified `ethcontract::MethodBuilder` encoded as a
/// `StorageAccessible` call.
///
/// # Panics
///
/// Panics if:
/// - The method doesn't specify a target address or calldata
/// - The function name doesn't exist or match the method signature
/// - The contract does not have deployment code
pub async fn simulate<T, R>(
    web3: &Web3<T>,
    contract: &ethcontract::Contract,
    function_name: &str,
    method: MethodBuilder<T, R>,
) -> Result<R, MethodError>
where
    T: Transport,
    R: Tokenize,
{
    let function = contract.abi.function(function_name).unwrap();
    let code = contract.bytecode.to_bytes().unwrap();

    let call = call(method.tx.to.unwrap(), code, method.tx.data.clone().unwrap());
    let output = web3
        .eth()
        .call(call, None)
        .await
        .map_err(|err| MethodError::new(function, err))?;

    let tokens = function
        .decode_output(&output.0)
        .map_err(|err| MethodError::new(function, err))?;
    R::from_token(abi::Token::Tuple(tokens)).map_err(|err| MethodError::new(function, err))
}
