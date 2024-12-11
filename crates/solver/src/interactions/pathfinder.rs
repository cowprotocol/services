use anyhow::{anyhow, Result};
use ethcontract::{H160, U256};
use serde::Deserialize;
use std::str::FromStr;

#[derive(Debug, Deserialize)]
pub struct TransferStep {
    pub from: H160,
    pub to: H160,
    pub token_owner: H160,
    pub value: String,
}

#[derive(Debug, Deserialize)]
struct PathfinderResult {
    #[serde(rename = "maxFlowValue")]
    pub max_flow_value: String,
    #[serde(rename = "final")]
    pub final_result: bool,
    #[serde(rename = "transferSteps")]
    pub transfer_steps: Vec<TransferStep>,
}

#[derive(Debug, Deserialize)]
struct PathfinderResponse {
    pub jsonrpc: String,
    pub id: String,
    pub result: PathfinderResult,
}

/// Fetch transfer steps from a pathfinder API.
/// 
/// # Arguments
/// - `endpoint`: The pathfinder endpoint URL (e.g., "https://example.com/pathfinder").
/// - `input_token`: The token we're sending.
/// - `output_token`: The token we want to receive.
/// - `src`: The starting address (source).
/// - `dest`: The ending address (destination).
/// - `amount`: The amount we want to route.
///
/// # Returns
/// A vector of `TransferStep` if successful. Returns an error if no route is found or if the request fails.
pub async fn fetch_transfer_steps(
    endpoint: &str,
    input_token: H160,
    output_token: H160,
    src: H160,
    dest: H160,
    amount: U256,
) -> Result<Vec<TransferStep>> {
    // Construct the request payload
    let body = serde_json::json!({
        "jsonrpc": "2.0",
        "id": "1",
        "method": "findRoute",
        "params": {
            "inputToken": format!("{:#x}", input_token),
            "outputToken": format!("{:#x}", output_token),
            "src": format!("{:#x}", src),
            "dest": format!("{:#x}", dest),
            "amount": amount.to_string()
        }
    });

    let client = reqwest::Client::new();
    let resp = client
        .post(endpoint)
        .json(&body)
        .send()
        .await
        .map_err(|e| anyhow!("Failed to send pathfinder request: {:?}", e))?;

    if !resp.status().is_success() {
        return Err(anyhow!(
            "Pathfinder request failed with status: {}",
            resp.status()
        ));
    }

    let parsed: PathfinderResponse = resp.json().await.map_err(|e| {
        anyhow!(
            "Failed to parse pathfinder response. Error: {:?}. Response body was not valid JSON.",
            e
        )
    })?;

    // Validate that transferSteps are present and non-empty
    if parsed.result.transfer_steps.is_empty() {
        return Err(anyhow!("No transfer steps returned by pathfinder."));
    }

    // Check if the maxFlowValue is >= amount requested
    let max_flow = U256::from_str(&parsed.result.max_flow_value)
        .map_err(|_| anyhow!("Invalid maxFlowValue in pathfinder response"))?;
    if max_flow < amount {
        return Err(anyhow!(
            "Pathfinder returned maxFlowValue less than requested amount."
        ));
    }

    // Return the steps
    Ok(parsed.result.transfer_steps)
}

#[cfg(test)]
mod tests {
    use super::*;
    use httpmock::prelude::*;

    #[tokio::test]
    async fn test_fetch_transfer_steps_success() {
        let server = MockServer::start();

        let mock_response = serde_json::json!({
            "jsonrpc": "2.0",
            "id": "1",
            "result": {
                "maxFlowValue": "100",
                "final": false,
                "transferSteps": [
                    {
                        "from": "0x9BA1Bcd88E99d6E1E03252A70A63FEa83Bf1208c",
                        "to": "0x42cEDde51198D1773590311E2A340DC06B24cB37",
                        "token_owner": "0x9a0bbbbd3789f184CA88f2F6A40F42406cb842AC",
                        "value": "100"
                    }
                ]
            }
        });

        let _mock = server.mock(|when, then| {
            when.method(POST);
            then.status(200)
                .json_body(mock_response);
        });

        let input_token = H160::from_low_u64_be(1);
        let output_token = H160::from_low_u64_be(2);
        let src = H160::from_low_u64_be(3);
        let dest = H160::from_low_u64_be(4);
        let amount = U256::from(50);

        let steps = fetch_transfer_steps(&server.url(""), input_token, output_token, src, dest, amount)
            .await
            .expect("Should fetch steps successfully");
        assert_eq!(steps.len(), 1);
        assert_eq!(format!("{:x}", steps[0].from), "9ba1bcd88e99d6e1e03252a70a63fea83bf1208c");
    }

    #[tokio::test]
    async fn test_fetch_transfer_steps_no_steps() {
        let server = MockServer::start();

        let mock_response = serde_json::json!({
            "jsonrpc": "2.0",
            "id": "1",
            "result": {
                "maxFlowValue": "100",
                "final": true,
                "transferSteps": []
            }
        });

        let _mock = server.mock(|when, then| {
            when.method(POST);
            then.status(200)
                .json_body(mock_response);
        });

        let input_token = H160::from_low_u64_be(1);
        let output_token = H160::from_low_u64_be(2);
        let src = H160::from_low_u64_be(3);
        let dest = H160::from_low_u64_be(4);
        let amount = U256::from(100);

        let err = fetch_transfer_steps(&server.url(""), input_token, output_token, src, dest, amount)
            .await
            .unwrap_err();
        assert!(format!("{:?}", err).contains("No transfer steps returned"));
    }

    #[tokio::test]
    async fn test_fetch_transfer_steps_insufficient_flow() {
        let server = MockServer::start();

        let mock_response = serde_json::json!({
            "jsonrpc": "2.0",
            "id": "1",
            "result": {
                "maxFlowValue": "10",
                "final": true,
                "transferSteps": [
                    {
                        "from": "0x9BA1Bcd88E99d6E1E03252A70A63FEa83Bf1208c",
                        "to": "0x42cEDde51198D1773590311E2A340DC06B24cB37",
                        "token_owner": "0x9a0bbbbd3789f184CA88f2F6A40F42406cb842AC",
                        "value": "10"
                    }
                ]
            }
        });

        let _mock = server.mock(|when, then| {
            when.method(POST);
            then.status(200)
                .json_body(mock_response);
        });

        let input_token = H160::from_low_u64_be(1);
        let output_token = H160::from_low_u64_be(2);
        let src = H160::from_low_u64_be(3);
        let dest = H160::from_low_u64_be(4);
        let amount = U256::from(100); // request more than maxFlowValue

        let err = fetch_transfer_steps(&server.url(""), input_token, output_token, src, dest, amount)
            .await
            .unwrap_err();
        assert!(format!("{:?}", err).contains("less than requested amount"));
    }
} 