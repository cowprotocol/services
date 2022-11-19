//! Multicall encoding and decoding.

use crate::ethrpc::dummy;
use contracts::support::Multicall;
use ethcontract::{
    errors::ExecutionError,
    tokens::{self, Tokenize as _},
};
use hex_literal::hex;
use lazy_static::lazy_static;
use std::iter;
use web3::{
    self,
    api::Eth,
    ethabi::{self, ParamType, Token},
    types::{AccessList, BlockId, Bytes, CallRequest, H160, U256, U64},
    Transport,
};

/// A single call in a multicall batch.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Call {
    /// The address to call.
    pub to: H160,

    /// Optional gas limit to use for the call.
    pub gas: Option<U256>,

    /// Optional value to use for the call.
    pub value: Option<U256>,

    /// Data to use for the call.
    pub data: Bytes,
}

impl Call {
    /// Encode into a tuple used for ABI encoding.
    pub fn encode(self) -> (H160, U256, U256, ethcontract::Bytes<Vec<u8>>) {
        (
            self.to,
            self.gas.unwrap_or_default(),
            self.value.unwrap_or_default(),
            ethcontract::Bytes(self.data.0),
        )
    }
}

/// Additional options that affect all calls in the multicall.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Options {
    /// The transaction origin.
    ///
    /// Note that this will not be the `msg.sender` for the calls, which will
    /// instead be the Multicall trampoline contract.
    pub origin: Option<H160>,

    /// Total gas limit for all the calls.
    pub gas: Option<U256>,

    /// Gas price.
    pub gas_price: Option<U256>,

    /// The transaction type index.
    pub transaction_type: Option<U64>,

    /// Access list.
    pub access_list: Option<AccessList>,

    /// Max fee per gas.
    pub max_fee_per_gas: Option<U256>,

    /// Miner bribe.
    pub max_priority_fee_per_gas: Option<U256>,
}

/// Web3 convenience multicall extension.
#[async_trait::async_trait]
pub trait MulticallExt<T> {
    async fn multicall(
        &self,
        calls: Vec<Call>,
        options: Options,
        block: Option<BlockId>,
    ) -> Vec<Result<Bytes, ExecutionError>>;
}

#[async_trait::async_trait]
impl<T> MulticallExt<T> for Eth<T>
where
    T: Transport + Sync,
    T::Out: Send,
{
    async fn multicall(
        &self,
        calls: Vec<Call>,
        options: Options,
        block: Option<BlockId>,
    ) -> Vec<Result<Bytes, ExecutionError>> {
        let len = calls.len();
        let value = calls.iter().flat_map(|call| call.value).max();

        println!("0x{}", hex::encode(encode(calls.clone()).0));

        let return_data = match self
            .call(
                CallRequest {
                    from: options.origin,
                    to: None,
                    gas: options.gas,
                    gas_price: options.gas_price,
                    value,
                    data: Some(encode(calls)),
                    transaction_type: options.transaction_type,
                    access_list: options.access_list,
                    max_fee_per_gas: options.max_fee_per_gas,
                    max_priority_fee_per_gas: options.max_priority_fee_per_gas,
                },
                block,
            )
            .await
        {
            Ok(value) => value,
            Err(err) => return repeat_err(err, len),
        };

        decode(len, return_data)
    }
}

fn encode(calls: Vec<Call>) -> Bytes {
    // Unfortunately, `ethcontract` generated code requires a `Web3` instance
    // even if it isn't used - so lets make a dummy one.
    let web3 = dummy::web3();
    Multicall::builder(&web3, calls.into_iter().map(Call::encode).collect())
        .into_inner()
        .data
        .unwrap()
}

fn decode(len: usize, return_data: Bytes) -> Vec<Result<Bytes, ExecutionError>> {
    let results = match decode_return_data(len, return_data) {
        Ok(value) => value,
        Err(err) => return repeat_err(err, len),
    };

    results
        .into_iter()
        .map(|(success, data)| match success {
            true => Ok(Bytes(data.0)),
            false => Err(ExecutionError::Revert(decode_revert_reason(&data.0))),
        })
        .collect()
}

type ReturnData = Vec<(bool, ethcontract::Bytes<Vec<u8>>)>;

fn decode_return_data(len: usize, return_data: Bytes) -> Result<ReturnData, DecodeError> {
    lazy_static! {
        static ref KIND: [ParamType; 1] = [ParamType::Array(Box::new(ParamType::Tuple(vec![
            ParamType::Bool,
            ParamType::Bytes,
        ])),)];
    }

    let tokens = ethabi::decode(&*KIND, &return_data.0)?;
    let (results,) = <(ReturnData,)>::from_token(Token::Tuple(tokens))?;
    if results.len() != len {
        return Err(DecodeError);
    }

    Ok(results)
}

#[derive(Clone)]
struct DecodeError;

impl From<ethabi::Error> for DecodeError {
    fn from(_: ethabi::Error) -> Self {
        Self
    }
}

impl From<tokens::Error> for DecodeError {
    fn from(_: tokens::Error) -> Self {
        Self
    }
}

impl From<DecodeError> for ExecutionError {
    fn from(_: DecodeError) -> Self {
        ExecutionError::AbiDecode(ethabi::Error::InvalidData)
    }
}

fn decode_revert_reason(revert_data: &[u8]) -> Option<String> {
    const ERROR_SELECTOR: [u8; 4] = hex!("08c379a0");

    let bytes = revert_data.strip_prefix(&ERROR_SELECTOR)?;
    let mut tokens = ethabi::decode(&[ParamType::String], bytes).ok()?;
    match tokens.pop() {
        Some(Token::String(value)) if tokens.is_empty() => Some(value),
        _ => None,
    }
}

fn repeat_err<T, E>(err: E, len: usize) -> Vec<Result<T, ExecutionError>>
where
    E: Clone + Into<ExecutionError>,
{
    iter::repeat(err).map(E::into).map(Err).take(len).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ethrpc::{create_env_test_transport, Web3};
    use std::fmt::Debug;

    #[test]
    fn encode_multicall() {
        let encoded = encode(vec![
            Call {
                to: H160([1; 20]),
                data: Bytes(vec![1, 2]),
                ..Default::default()
            },
            Call {
                to: H160([2; 20]),
                gas: Some(42.into()),
                value: Some(1337.into()),
                data: Bytes(vec![3, 4]),
            },
        ]);

        let expected = [
            &bytecode!(Multicall).0[..],
            &hex!(
                "0000000000000000000000000000000000000000000000000000000000000020
                 0000000000000000000000000000000000000000000000000000000000000002
                 0000000000000000000000000000000000000000000000000000000000000040
                 0000000000000000000000000000000000000000000000000000000000000100
                 0000000000000000000000000101010101010101010101010101010101010101
                 0000000000000000000000000000000000000000000000000000000000000000
                 0000000000000000000000000000000000000000000000000000000000000000
                 0000000000000000000000000000000000000000000000000000000000000080
                 0000000000000000000000000000000000000000000000000000000000000002
                 0102000000000000000000000000000000000000000000000000000000000000
                 0000000000000000000000000202020202020202020202020202020202020202
                 000000000000000000000000000000000000000000000000000000000000002a
                 0000000000000000000000000000000000000000000000000000000000000539
                 0000000000000000000000000000000000000000000000000000000000000080
                 0000000000000000000000000000000000000000000000000000000000000002
                 0304000000000000000000000000000000000000000000000000000000000000"
            ),
        ]
        .concat();

        assert_eq!(encoded.0, expected);
    }

    #[test]
    fn decode_multicall() {
        let decoded = decode(
            3,
            bytes!(
                "0000000000000000000000000000000000000000000000000000000000000020
                 0000000000000000000000000000000000000000000000000000000000000003
                 0000000000000000000000000000000000000000000000000000000000000060
                 00000000000000000000000000000000000000000000000000000000000000e0
                 0000000000000000000000000000000000000000000000000000000000000140
                 0000000000000000000000000000000000000000000000000000000000000001
                 0000000000000000000000000000000000000000000000000000000000000040
                 0000000000000000000000000000000000000000000000000000000000000002
                 0102000000000000000000000000000000000000000000000000000000000000
                 0000000000000000000000000000000000000000000000000000000000000000
                 0000000000000000000000000000000000000000000000000000000000000040
                 0000000000000000000000000000000000000000000000000000000000000000
                 0000000000000000000000000000000000000000000000000000000000000000
                 0000000000000000000000000000000000000000000000000000000000000040
                 0000000000000000000000000000000000000000000000000000000000000064
                 08c379a0
                 0000000000000000000000000000000000000000000000000000000000000020
                 0000000000000000000000000000000000000000000000000000000000000004
                 706f6f7000000000000000000000000000000000000000000000000000000000
                         00000000000000000000000000000000000000000000000000000000"
            ),
        );

        assert_result_eq(
            &decoded,
            &[
                Ok(bytes!("0102")),
                Err(ExecutionError::Revert(None)),
                Err(ExecutionError::Revert(Some("poop".to_owned()))),
            ],
        );
    }

    #[ignore]
    #[tokio::test]
    async fn execute_multicall() {
        let web3 = Web3::new(create_env_test_transport());
        let results = web3
            .eth()
            .multicall(
                vec![
                    // Get the token name
                    Call {
                        to: testlib::tokens::COW,
                        // name()
                        data: bytes!("06fdde03"),
                        ..Default::default()
                    },
                    // Revert because COW token doesn't accept ETH transfers
                    Call {
                        to: testlib::tokens::COW,
                        value: Some(1.into()),
                        data: bytes!(""),
                        ..Default::default()
                    },
                    // Revert because Multicall address doesn't have balance
                    Call {
                        to: testlib::tokens::COW,
                        // transfer(settlement, 1.0)
                        data: bytes!(
                            "a9059cbb
                             0000000000000000000000009008d19f58aabd9ed0d60971565aa8510560ab41
                             0000000000000000000000000000000000000000000000000de0b6b3a7640000"
                        ),
                        ..Default::default()
                    },
                    // Deposit some WETH
                    Call {
                        to: testlib::tokens::WETH,
                        // deposit()
                        value: Some(1.into()),
                        data: bytes!("d0e30db0"),
                        ..Default::default()
                    },
                ],
                Options::default(),
                None,
            )
            .await;

        assert_result_eq(
            &results,
            &[
                // "CoW Protocol Token"
                Ok(bytes!(
                    "0000000000000000000000000000000000000000000000000000000000000020
                     0000000000000000000000000000000000000000000000000000000000000012
                     436f572050726f746f636f6c20546f6b656e0000000000000000000000000000"
                )),
                Err(ExecutionError::Revert(None)),
                Err(ExecutionError::Revert(Some(
                    "ERC20: transfer amount exceeds balance".to_owned(),
                ))),
                Ok(bytes!("")),
            ],
        );
    }

    #[ignore]
    #[tokio::test]
    async fn calls_are_unrelated() {
        let web3 = Web3::new(create_env_test_transport());
        let results = web3
            .eth()
            .multicall(
                vec![
                    // Deposit WETH
                    Call {
                        to: testlib::tokens::WETH,
                        value: Some(1.into()),
                        data: bytes!(""),
                        ..Default::default()
                    },
                    // Withdraw the WETH
                    Call {
                        to: testlib::tokens::WETH,
                        data: bytes!(
                            "2e1a7d4d
                             0000000000000000000000000000000000000000000000000000000000000001"
                        ),
                        ..Default::default()
                    },
                ],
                Options {
                    origin: Some(H160::zero()),
                    ..Default::default()
                },
                None,
            )
            .await;

        assert_result_eq(
            &results,
            &[
                // Deposit is successful, which increase the balance of the
                // Multicall contract.
                Ok(bytes!("")),
                // But the withdraw fails because the balance increase does not
                // affect the second call.
                Err(ExecutionError::Revert(None)),
            ],
        );
    }

    fn assert_result_eq<T>(
        actual: &[Result<T, ExecutionError>],
        expected: &[Result<T, ExecutionError>],
    ) where
        T: Debug + PartialEq,
    {
        for (actual, expected) in actual.iter().zip(expected) {
            match (actual, expected) {
                (Ok(a), Ok(b)) if a == b => (),
                (Err(ExecutionError::Revert(a)), Err(ExecutionError::Revert(b))) if a == b => (),
                _ => {
                    panic!("{actual:?} != {expected:?}");
                }
            };
        }
    }
}
