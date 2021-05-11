use anyhow::{anyhow, Result};
use serde_json::Value;
use shared::Web3;
use web3::{
    types::{BlockNumber, BlockTrace, CallRequest, TraceType},
    Transport,
};

// Use the trace_callMany api https://openethereum.github.io/JSONRPC-trace-module#trace_callmany
// api to simulate whether these call requests if applied together in order would succeed.
// Err if communication with the node failed.
// Ok(true) if transactions simulate without reverting
// Ok(false) if transactions simulate with at least one revert.
pub async fn trace_many(requests: Vec<CallRequest>, web3: &Web3) -> Result<bool> {
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
    handle_trace_many_response(response)
}

// Same return value as above but factored out for easier testing.
fn handle_trace_many_response(response: Value) -> Result<bool> {
    let traces: Vec<BlockTrace> = serde_json::from_value(response)?;
    for trace in traces {
        let transaction_trace = trace.trace.ok_or_else(|| anyhow!("trace not set"))?;
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
    fn communication_fails() {
        let result = handle_trace_many_response(Value::Null);
        assert!(result.is_err());

        let response = json!([{"output": "0x", "trace": []}]);
        let result = handle_trace_many_response(response);
        assert!(result.is_err());
    }

    #[test]
    fn ok_true() {
        let response = json!(
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
          }]);

        let result = handle_trace_many_response(response);
        assert!(result.unwrap());
    }

    #[test]
    fn ok_false() {
        let response = json!(
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
          }]);

        let result = handle_trace_many_response(response);
        assert!(!result.unwrap());
    }
}
