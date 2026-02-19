use {
    super::TokenQuality,
    crate::{trace_many, web3::Web3},
    alloy::{
        primitives::{Address, U256, keccak256},
        rpc::{
            json_rpc::ErrorPayload,
            types::{
                TransactionRequest,
                trace::parity::{TraceOutput, TraceResults},
            },
        },
        signers::local::PrivateKeySigner,
        sol_types::SolCall,
        transports::{RpcError, TransportErrorKind},
    },
    anyhow::{Context, Result, bail, ensure},
    contracts::alloy::ERC20,
    model::interaction::InteractionData,
};

const METHOD_NOT_FOUND_CODE: i64 = -32601;

/// Detects whether a token is "bad" (works in unexpected ways that are
/// problematic for solving) by simulating several transfers of a token.
#[derive(Debug, Clone)]
pub struct TraceCallDetectorRaw {
    pub web3: Web3,
    pub settlement_contract: Address,
}

impl TraceCallDetectorRaw {
    pub fn new(web3: Web3, settlement: Address) -> Self {
        Self {
            web3,
            settlement_contract: settlement,
        }
    }

    pub async fn test_transfer(
        &self,
        take_from: Address,
        token: Address,
        amount: U256,
        pre_interactions: &[InteractionData],
    ) -> Result<TokenQuality> {
        let mut request: Vec<_> = pre_interactions
            .iter()
            .map(|i| {
                TransactionRequest::default()
                    .to(i.target)
                    .value(i.value)
                    .input(i.call_data.clone().into())
            })
            .collect();
        // We transfer the full available amount of the token from the amm pool into the
        // settlement contract and then to an arbitrary address.
        // Note that gas use can depend on the recipient because for the standard
        // implementation sending to an address that does not have any balance
        // yet (implicitly 0) causes an allocation.
        request.append(&mut self.create_trace_request(token, amount, take_from));
        let traces = match trace_many::trace_many(&self.web3, request).await {
            Ok(result) => result,
            Err(RpcError::UnsupportedFeature(err)) => {
                tracing::warn!(error = ?err, "node does not support trace_callMany");
                return Ok(TokenQuality::Good);
            }
            Err(RpcError::ErrorResp(ErrorPayload {
                code: METHOD_NOT_FOUND_CODE,
                message,
                ..
            })) => {
                tracing::warn!(error = %message, "node does not support trace_callMany");
                return Ok(TokenQuality::Good);
            }
            Err(RpcError::Transport(TransportErrorKind::HttpError(err))) if err.status == 400 => {
                tracing::warn!(
                    error=?err,
                    "unable to perform trace call with configured node, assume good quality"
                );
                return Ok(TokenQuality::Good);
            }
            Err(e) => {
                return Err(e).context("trace_many");
            }
        };
        let relevant_traces = &traces[pre_interactions.len()..];
        Self::handle_response(relevant_traces, amount, take_from)
    }

    // For the out transfer we use an arbitrary address without balance to detect
    // tokens that usually apply fees but not if the the sender or receiver is
    // specifically exempt like their own uniswap pools.
    fn arbitrary_recipient() -> Address {
        PrivateKeySigner::from_bytes(&keccak256(b"moo"))
            .unwrap()
            .address()
    }

    fn create_trace_request(
        &self,
        token: Address,
        amount: U256,
        take_from: Address,
    ) -> Vec<TransactionRequest> {
        let mut requests = Vec::new();
        let recipient = Self::arbitrary_recipient();
        let settlement_contract = self.settlement_contract;

        // 0
        let calldata = ERC20::ERC20::balanceOfCall {
            account: settlement_contract,
        }
        .abi_encode();
        requests.push(
            TransactionRequest::default()
                .to(token)
                .input(calldata.into()),
        );
        // 1
        let calldata = ERC20::ERC20::transferCall {
            recipient: settlement_contract,
            amount,
        }
        .abi_encode();
        requests.push(
            TransactionRequest::default()
                .from(take_from)
                .to(token)
                .input(calldata.into()),
        );
        // 2
        let calldata = ERC20::ERC20::balanceOfCall {
            account: settlement_contract,
        }
        .abi_encode();
        requests.push(
            TransactionRequest::default()
                .to(token)
                .input(calldata.into()),
        );
        // 3
        let calldata = ERC20::ERC20::balanceOfCall { account: recipient }.abi_encode();
        requests.push(
            TransactionRequest::default()
                .to(token)
                .input(calldata.into()),
        );
        // 4
        let calldata = ERC20::ERC20::transferCall { recipient, amount }.abi_encode();
        requests.push(
            TransactionRequest::default()
                .from(self.settlement_contract)
                .to(token)
                .input(calldata.into()),
        );
        // 5
        let calldata = ERC20::ERC20::balanceOfCall {
            account: settlement_contract,
        }
        .abi_encode();
        requests.push(
            TransactionRequest::default()
                .to(token)
                .input(calldata.into()),
        );
        // 6
        let calldata = ERC20::ERC20::balanceOfCall { account: recipient }.abi_encode();
        requests.push(
            TransactionRequest::default()
                .to(token)
                .input(calldata.into()),
        );

        // 7
        let calldata = ERC20::ERC20::approveCall {
            spender: recipient,
            amount: alloy::primitives::U256::MAX,
        }
        .abi_encode();
        requests.push(
            TransactionRequest::default()
                .from(self.settlement_contract)
                .to(token)
                .input(calldata.into()),
        );

        requests
    }

    fn handle_response(
        traces: &[TraceResults],
        amount: U256,
        take_from: Address,
    ) -> Result<TokenQuality> {
        ensure!(traces.len() == 8, "unexpected number of traces");

        let gas_in = match ensure_transaction_ok_and_get_gas(&traces[1])? {
            Ok(gas) => gas,
            Err(reason) => {
                return Ok(TokenQuality::bad(format!(
                    "Transfer of token from on chain source {take_from:?} into settlement \
                     contract failed: {reason}"
                )));
            }
        };
        let arbitrary = Self::arbitrary_recipient();
        let gas_out = match ensure_transaction_ok_and_get_gas(&traces[4])? {
            Ok(gas) => gas,
            Err(reason) => {
                return Ok(TokenQuality::bad(format!(
                    "Transfer token out of settlement contract to arbitrary recipient \
                     {arbitrary:?} failed: {reason}",
                )));
            }
        };

        let message = "\
            Failed to decode the token's balanceOf response because it did not \
            return 32 bytes. A common cause of this is a bug in the Vyper \
            smart contract compiler. See \
            https://github.com/cowprotocol/services/pull/781 for more \
            information.\
        ";
        let bad = TokenQuality::Bad {
            reason: message.to_string(),
        };
        let balance_before_in = match u256_from_be_bytes_strict(&traces[0].output) {
            Some(balance) => balance,
            None => return Ok(bad),
        };
        let balance_after_in = match u256_from_be_bytes_strict(&traces[2].output) {
            Some(balance) => balance,
            None => return Ok(bad),
        };
        let balance_after_out = match u256_from_be_bytes_strict(&traces[5].output) {
            Some(balance) => balance,
            None => return Ok(bad),
        };
        let balance_recipient_before = match u256_from_be_bytes_strict(&traces[3].output) {
            Some(balance) => balance,
            None => return Ok(bad),
        };
        let balance_recipient_after = match u256_from_be_bytes_strict(&traces[6].output) {
            Some(balance) => balance,
            None => return Ok(bad),
        };

        tracing::debug!(%amount, %balance_before_in, %balance_after_in, %balance_after_out);

        let computed_balance_after_in = match balance_before_in.checked_add(amount) {
            Some(amount) => amount,
            None => {
                return Ok(TokenQuality::bad(format!(
                    "Transferring {amount} into settlement contract would overflow its balance."
                )));
            }
        };
        // Allow for a small discrepancy (1 wei) in the balance after the transfer which
        // may come from rounding discrepancies in tokens that track balances
        // with "shares" (e.g. eUSD).
        if balance_after_in < computed_balance_after_in.saturating_sub(U256::ONE) {
            return Ok(TokenQuality::bad(format!(
                "Transferring {amount} into settlement contract was expected to result in a \
                 balance of {computed_balance_after_in} but actually resulted in \
                 {balance_after_in}. A common cause for this is that the token takes a fee on \
                 transfer."
            )));
        }
        if balance_after_out != balance_before_in {
            return Ok(TokenQuality::bad(format!(
                "Transferring {amount} out of settlement contract was expected to result in the \
                 original balance of {balance_before_in} but actually resulted in \
                 {balance_after_out}."
            )));
        }
        let computed_balance_recipient_after = match balance_recipient_before.checked_add(amount) {
            Some(amount) => amount,
            None => {
                return Ok(TokenQuality::bad(format!(
                    "Transferring {amount} into arbitrary recipient {arbitrary:?} would overflow \
                     its balance."
                )));
            }
        };
        // Allow for a small discrepancy (1 wei) in the balance after the transfer
        // which may come from rounding discrepancies in tokens that track
        // balances with "shares" (e.g. eUSD).
        if computed_balance_recipient_after < balance_recipient_after.saturating_sub(U256::ONE) {
            return Ok(TokenQuality::bad(format!(
                "Transferring {amount} into arbitrary recipient {arbitrary:?} was expected to \
                 result in a balance of {computed_balance_recipient_after} but actually resulted \
                 in {balance_recipient_after}. A common cause for this is that the token takes a \
                 fee on transfer."
            )));
        }

        if let Err(err) = ensure_transaction_ok_and_get_gas(&traces[7])? {
            return Ok(TokenQuality::bad(format!(
                "Approval of U256::MAX failed: {err}"
            )));
        }

        let _gas_per_transfer = (gas_in + gas_out) / U256::from(2);
        Ok(TokenQuality::Good)
    }
}

/// The outer result signals communication failure with the node.
/// The inner result is Ok(gas_price) or Err if the transaction failed.
fn ensure_transaction_ok_and_get_gas(trace: &TraceResults) -> Result<Result<U256, String>> {
    let first = trace.trace.first().context("expected at least one trace")?;
    if let Some(error) = &first.error {
        return Ok(Err(format!("transaction failed: {error}")));
    }
    let call_result = match &first.result {
        Some(TraceOutput::Call(call)) => call,
        _ => bail!("no error but also no call result"),
    };
    Ok(Ok(U256::from(call_result.gas_used)))
}

/// Decodes a `U256` from a big-endian encoded slice.
/// The slice's length MUST be 32 bytes.
fn u256_from_be_bytes_strict(b: &[u8]) -> Option<U256> {
    if b.len() != 32 {
        return None;
    }
    U256::try_from_be_slice(b)
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        alloy::{
            primitives::Bytes,
            rpc::types::trace::parity::{
                Action,
                CallAction,
                CallOutput,
                CallType,
                TransactionTrace,
            },
        },
    };

    #[test]
    fn handle_response_ok() {
        let traces = &[
            TraceResults {
                output: U256::ZERO.to_be_bytes::<32>().to_vec().into(),
                trace: vec![],
                vm_trace: None,
                state_diff: None,
            },
            TraceResults {
                output: Default::default(),
                trace: vec![TransactionTrace {
                    trace_address: Vec::new(),
                    subtraces: 0,
                    action: Action::Call(CallAction {
                        from: Address::ZERO,
                        to: Address::ZERO,
                        value: U256::ZERO,
                        gas: 0,
                        input: Bytes::new(),
                        call_type: CallType::None,
                    }),
                    result: Some(TraceOutput::Call(CallOutput {
                        gas_used: 1,
                        output: Bytes::new(),
                    })),
                    error: None,
                }],
                vm_trace: None,
                state_diff: None,
            },
            TraceResults {
                output: U256::ONE.to_be_bytes::<32>().to_vec().into(),
                trace: vec![],
                vm_trace: None,
                state_diff: None,
            },
            TraceResults {
                output: U256::ZERO.to_be_bytes::<32>().to_vec().into(),
                trace: vec![],
                vm_trace: None,
                state_diff: None,
            },
            TraceResults {
                output: Default::default(),
                trace: vec![TransactionTrace {
                    trace_address: Vec::new(),
                    subtraces: 0,
                    action: Action::Call(CallAction {
                        from: Address::ZERO,
                        to: Address::ZERO,
                        value: U256::ZERO,
                        gas: 0,
                        input: Bytes::new(),
                        call_type: CallType::None,
                    }),
                    result: Some(TraceOutput::Call(CallOutput {
                        gas_used: 3,
                        output: Bytes::new(),
                    })),
                    error: None,
                }],
                vm_trace: None,
                state_diff: None,
            },
            TraceResults {
                output: U256::ZERO.to_be_bytes::<32>().to_vec().into(),
                trace: vec![],
                vm_trace: None,
                state_diff: None,
            },
            TraceResults {
                output: U256::ONE.to_be_bytes::<32>().to_vec().into(),
                trace: vec![],
                vm_trace: None,
                state_diff: None,
            },
            TraceResults {
                output: Default::default(),
                trace: vec![TransactionTrace {
                    trace_address: Vec::new(),
                    subtraces: 0,
                    action: Action::Call(CallAction {
                        from: Address::ZERO,
                        to: Address::ZERO,
                        value: U256::ZERO,
                        gas: 0,
                        input: Bytes::new(),
                        call_type: CallType::None,
                    }),
                    result: Some(TraceOutput::Call(CallOutput {
                        gas_used: 1,
                        output: Bytes::new(),
                    })),
                    error: None,
                }],
                vm_trace: None,
                state_diff: None,
            },
        ];

        let result =
            TraceCallDetectorRaw::handle_response(traces, U256::ONE, Address::ZERO).unwrap();
        let expected = TokenQuality::Good;
        assert_eq!(result, expected);
    }
}
