use crate::Web3;
use anyhow::{anyhow, Context, Result};
use web3::{
    types::{BlockNumber, BlockTrace, CallRequest, TraceType},
    Transport,
};

// Use the trace_callMany api https://openethereum.github.io/JSONRPC-trace-module#trace_callmany
// api to simulate these call requests applied together one after another.
// Err if communication with the node failed.
pub async fn trace_many(requests: Vec<CallRequest>, web3: &Web3) -> Result<Vec<BlockTrace>> {
    let transport = web3.transport();
    let requests = requests
        .into_iter()
        .map(|request| {
            Ok(vec![
                serde_json::to_value(request)?,
                serde_json::to_value(vec![TraceType::Trace])?,
            ])
        })
        .collect::<Result<Vec<_>>>()?;
    let block = BlockNumber::Latest;
    let params = vec![
        serde_json::to_value(requests)?,
        serde_json::to_value(block)?,
    ];
    let response = transport.execute("trace_callMany", params).await?;
    serde_json::from_value(response).context("failed to decode response")
}

// Check the return value of trace_many for whether all top level transactions succeeded (did not
// revert).
// Err if the response is missing trace data.
// Ok(true) if transactions simulate without reverting
// Ok(false) if transactions simulate with at least one revert.
pub fn all_calls_succeeded(traces: &[BlockTrace]) -> Result<bool> {
    for trace in traces {
        let transaction_trace = trace
            .trace
            .as_ref()
            .ok_or_else(|| anyhow!("trace not set"))?;
        let first = transaction_trace
            .first()
            .ok_or_else(|| anyhow!("expected at least one trace"))?;
        if first.error.is_some() {
            return Ok(false);
        }
    }
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn ok_true() {
        let response: Vec<BlockTrace> = serde_json::from_value(json!(
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
        let response: Vec<BlockTrace> = serde_json::from_value(json!(
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
