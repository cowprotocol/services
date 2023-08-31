//! Module to help encoding `eth_call`s for the `Reader.sol` contract.

use {
    crate::support::Reader,
    ethcontract::{
        common::abi,
        tokens::Tokenize,
        web3::types::{Bytes, CallRequest},
        H160,
    },
};

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
                Reader::raw_contract()
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
