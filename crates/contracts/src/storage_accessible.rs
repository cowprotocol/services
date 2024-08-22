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
        },
        H160,
    },
    std::sync::LazyLock,
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

    // memoize value to skip hex-decodeing on every call
    static BYTECODE: LazyLock<Vec<u8>> =
        LazyLock::new(|| SimulateCode::raw_contract().bytecode.to_bytes().unwrap().0);

    CallRequest {
        data: Some([BYTECODE.as_slice(), &args].concat().into()),
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
pub async fn simulate<T, R>(code: Bytes, mut method: MethodBuilder<T, R>) -> Result<R, MethodError>
where
    T: Transport,
    R: Tokenize,
{
    method.tx.data = call(method.tx.to.unwrap(), code, method.tx.data.take().unwrap()).data;
    method.tx.to = None;
    method.call().await
}
