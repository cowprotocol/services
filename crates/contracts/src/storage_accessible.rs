//! Module to help encoding `eth_call`s for the `StorageAccessible` contracts.

use {
    crate::support::SimulateCode,
    ethcontract::{
        common::abi,
        tokens::Tokenize,
        web3::types::{Bytes, CallRequest},
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
