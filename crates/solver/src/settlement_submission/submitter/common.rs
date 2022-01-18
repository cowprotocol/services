use super::super::submitter::{SubmitApiError, TransactionHandle};
use anyhow::{anyhow, Result};
use ethcontract::{
    dyns::DynTransport,
    transaction::{Transaction, TransactionBuilder},
};
use futures::FutureExt;
use jsonrpc_core::Output;
use primitive_types::H256;
use reqwest::{Client, Url};
use serde::de::DeserializeOwned;

/// Function for sending raw signed transaction to private networks
pub async fn submit_raw_transaction(
    client: Client,
    url: Url,
    tx: TransactionBuilder<DynTransport>,
) -> Result<TransactionHandle, SubmitApiError> {
    let (raw_signed_transaction, tx_hash) = match tx.build().now_or_never().unwrap().unwrap() {
        Transaction::Request(_) => unreachable!("verified offline account was used"),
        Transaction::Raw { bytes, hash } => (bytes.0, hash),
    };
    let tx = format!("0x{}", hex::encode(raw_signed_transaction));
    let body = serde_json::json!({
      "jsonrpc": "2.0",
      "id": 1,
      "method": "eth_sendRawTransaction",
      "params": [tx],
    });
    tracing::debug!(
        "submit_transaction body: {}",
        serde_json::to_string(&body).unwrap_or_else(|err| format!("error: {:?}", err)),
    );
    let response = client
        .post(url.clone())
        .json(&body)
        .send()
        .await
        .map_err(|err| SubmitApiError::Other(err.into()))?;
    let body = response
        .text()
        .await
        .map_err(|err| SubmitApiError::Other(err.into()))?;

    let handle = parse_json_rpc_response::<H256>(&body)?;
    tracing::info!(
        "created transaction with hash: {:?} and handle: {:?}, url: {}",
        tx_hash,
        handle,
        url.as_str()
    );
    Ok(TransactionHandle { tx_hash, handle })
}

fn parse_json_rpc_response<T>(body: &str) -> Result<T, SubmitApiError>
where
    T: DeserializeOwned,
{
    match serde_json::from_str::<Output>(body) {
        Ok(output) => match output {
            Output::Success(body) => serde_json::from_value::<T>(body.result).map_err(|_| {
                anyhow!(
                    "failed conversion to expected type {}",
                    std::any::type_name::<T>()
                )
                .into()
            }),
            Output::Failure(body) => {
                if body.error.message.contains("invalid nonce") {
                    Err(SubmitApiError::InvalidNonce)
                } else if body
                    .error
                    .message
                    .contains("Transaction gas price supplied is too low")
                {
                    Err(SubmitApiError::OpenEthereumTooCheapToReplace)
                } else {
                    Err(anyhow!("rpc error: {}", body.error).into())
                }
            }
        },
        Err(_) => Err(anyhow!("invalid rpc response: {}", body).into()),
    }
}
