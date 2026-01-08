use {
    crate::ethrpc::Web3,
    alloy::{
        providers::ext::TraceApi,
        rpc::types::{
            TransactionRequest,
            trace::parity::{TraceResults, TraceType},
        },
        transports::{RpcError, TransportErrorKind},
    },
    anyhow::{Context, Result},
};

/// Use the trace_callMany API (<https://openethereum.github.io/JSONRPC-trace-module#trace_callmany>)
/// to simulate these call requests applied together one after another.
///
/// Returns `Err` if communication with the node failed.
pub async fn trace_many(
    web3: &Web3,
    requests: Vec<TransactionRequest>,
) -> Result<Vec<TraceResults>, RpcError<TransportErrorKind>> {
    let r: Vec<_> = requests
        .into_iter()
        .zip(std::iter::repeat([TraceType::Trace].as_slice()))
        .collect();

    web3.alloy.trace_call_many(r.as_slice()).latest().await
}

/// Check the return value of `trace_many` for whether all top level
/// transactions succeeded (did not revert).
///
/// * `Err` if the response is missing trace data.
/// * `Ok(true)` if transactions simulate without reverting
/// * `Ok(false)` if transactions simulate with at least one revert.
pub fn all_calls_succeeded(traces: &[TraceResults]) -> Result<bool> {
    for trace in traces {
        let first = trace.trace.first().context("expected at least one trace")?;
        if first.error.is_some() {
            return Ok(false);
        }
    }
    Ok(true)
}

#[cfg(test)]
mod tests {
    use {super::*, serde_json::json};

    #[test]
    fn ok_true() {
        let response: Vec<TraceResults> = serde_json::from_value(json!(
        [{
            "output": "0x",
            "trace": [{
              "traceAddress": [],
              "subtraces": 0,
              "action": {
                "callType": "call",
                "from": "0x0000000000000000000000000000000000000000",
                "gas": "0x00",
                "input": "0x",
                "to": "0x0000000000000000000000000000000000000000",
                "value": "0x00"
              },
              "type": "call"
            }],
          }]))
        .unwrap();
        let result = all_calls_succeeded(&response);
        assert!(result.unwrap());
    }

    #[test]
    fn ok_false() {
        let response: Vec<TraceResults> = serde_json::from_value(json!(
        [{
            "output": "0x",
            "trace": [{
              "traceAddress": [],
              "subtraces": 0,
              "action": {
                "callType": "call",
                "from": "0x0000000000000000000000000000000000000000",
                "gas": "0x00",
                "input": "0x",
                "to": "0x0000000000000000000000000000000000000000",
                "value": "0x00"
              },
              "type": "call",
              "error": "Reverted"
            }],
          }]))
        .unwrap();

        let result = all_calls_succeeded(&response);
        assert!(!result.unwrap());
    }
}
