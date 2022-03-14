use anyhow::{anyhow, ensure, Context, Result};
use ethcontract::{dyns::DynTransport, transaction::TransactionBuilder, Address, H160, H256};
use reqwest::{
    header::{HeaderMap, HeaderValue},
    Client, IntoUrl, Url,
};
use serde::{Deserialize, Serialize};
use shared::Web3;
use web3::{
    helpers,
    types::{AccessList, Bytes, CallRequest},
    BatchTransport, Transport,
};

#[async_trait::async_trait]
pub trait AccessListEstimating: Send + Sync {
    async fn estimate_access_list(
        &self,
        tx: &TransactionBuilder<DynTransport>,
    ) -> Result<AccessList> {
        self.estimate_access_lists(std::slice::from_ref(tx))
            .await?
            .into_iter()
            .next()
            .unwrap()
    }
    /// The function guarantees the same size and order of input and output containers
    async fn estimate_access_lists(
        &self,
        txs: &[TransactionBuilder<DynTransport>],
    ) -> Result<Vec<Result<AccessList>>>;
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct NodeAccessList {
    access_list: AccessList,
}

#[derive(Debug)]
pub struct NodeApi {
    web3: Web3,
}

impl NodeApi {
    pub fn new(web3: Web3) -> Self {
        Self { web3 }
    }
}

#[async_trait::async_trait]
impl AccessListEstimating for NodeApi {
    async fn estimate_access_lists(
        &self,
        txs: &[TransactionBuilder<DynTransport>],
    ) -> Result<Vec<Result<AccessList>>> {
        let batch_request = txs
            .iter()
            .map(|tx| -> Result<_> {
                let (from, to, data) = resolve_call_request(tx)?;
                let request = CallRequest {
                    from: Some(from),
                    to: Some(to),
                    data: Some(data),
                    ..Default::default()
                };
                let params = helpers::serialize(&request);
                let (id, request) = self
                    .web3
                    .transport()
                    .prepare("eth_createAccessList", vec![params]);
                Ok((id, request))
            })
            .collect::<Vec<_>>();

        // send_batch guarantees the size and order of the responses to match the requests
        let mut batch_response = self
            .web3
            .transport()
            .send_batch(batch_request.iter().flatten().cloned())
            .await?
            .into_iter();

        Ok(batch_request
            .into_iter()
            // merge results of unresolved call requests with responses of resolved requests
            .map(|req| match req {
                // error during `resolve_call_request()`
                Err(e) => Err(e),
                Ok(_req) => match batch_response.next().unwrap() {
                    Ok(response) => serde_json::from_value::<NodeAccessList>(response)
                        // error parsing the response
                        .context("unexpected response format")
                        .map(|response| response.access_list),
                    // error during transport
                    Err(err) => Err(anyhow!("web3 error: {}", err)),
                },
            })
            .collect())
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
struct TenderlyRequest {
    network_id: String,
    block_number: u64,
    from: Address,
    input: Bytes,
    to: Address,
    generate_access_list: bool,
}

#[derive(Debug, Clone, Deserialize)]
struct TenderlyResponse {
    generated_access_list: Vec<AccessListItem>,
}

// Had to introduce copy of the web3 AccessList because tenderly responds with snake_case fields
// and tenderly storage_keys field does not exist if empty (it should be empty Vec instead)
#[derive(Debug, Clone, Deserialize)]
struct AccessListItem {
    /// Accessed address
    address: Address,
    /// Accessed storage keys
    #[serde(default)]
    storage_keys: Vec<H256>,
}

impl From<AccessListItem> for web3::types::AccessListItem {
    fn from(item: AccessListItem) -> Self {
        Self {
            address: item.address,
            storage_keys: item.storage_keys,
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
struct BlockNumber {
    block_number: u64,
}

#[derive(Debug)]
pub struct TenderlyApi {
    url: Url,
    client: Client,
    header: HeaderMap,
    network_id: String,
}

impl TenderlyApi {
    #[allow(dead_code)]
    pub fn new(
        url: impl IntoUrl,
        client: Client,
        api_key: &str,
        network_id: String,
    ) -> Result<Self> {
        Ok(Self {
            url: url.into_url()?,
            client,
            header: {
                let mut header = HeaderMap::new();
                header.insert("x-access-key", HeaderValue::from_str(api_key)?);
                header
            },
            network_id,
        })
    }

    #[allow(dead_code)]
    async fn send(&self, body: TenderlyRequest) -> reqwest::Result<TenderlyResponse> {
        self.client
            .post(self.url.clone())
            .headers(self.header.clone())
            .json(&body)
            .send()
            .await?
            .error_for_status()?
            .json()
            .await
    }

    async fn block_number(&self, network_id: String) -> reqwest::Result<BlockNumber> {
        self.client
            .get(format!(
                "https://api.tenderly.co/api/v1/network/{}/block-number",
                network_id
            ))
            .send()
            .await?
            .error_for_status()?
            .json()
            .await
    }
}

#[async_trait::async_trait]
impl AccessListEstimating for TenderlyApi {
    async fn estimate_access_lists(
        &self,
        txs: &[TransactionBuilder<DynTransport>],
    ) -> Result<Vec<Result<AccessList>>> {
        Ok(futures::future::join_all(txs.iter().map(|tx| async {
            let (from, to, input) = resolve_call_request(tx)?;
            let block_number = self.block_number(self.network_id.clone()).await?;

            let request = TenderlyRequest {
                network_id: self.network_id.clone(),
                block_number: block_number.block_number,
                from,
                input,
                to,
                generate_access_list: true,
            };

            let response = self.send(request).await?;
            ensure!(
                !response.generated_access_list.is_empty(),
                "empty access list"
            );
            Ok(response
                .generated_access_list
                .into_iter()
                .map(Into::into)
                .collect())
        }))
        .await)
    }
}

fn resolve_call_request(tx: &TransactionBuilder<DynTransport>) -> Result<(H160, H160, Bytes)> {
    let from = tx
        .from
        .clone()
        .context("transaction from does not exist")?
        .address();
    let to = tx.to.context("transaction to does not exist")?;
    let data = tx.data.clone().context("transaction data does not exist")?;
    Ok((from, to, data))
}

// this function should remove duplicates and elements that are not useful
// currently only eliminating addresses and storages with value '1', that should probably represent the address of the
// precompiled contract for signature recovery: https://github.com/ethereum/go-ethereum/blob/70da74e73a182620a09bb0cfbff173e6d65d0518/core/vm/contracts.go#L84
// for some reason it happens that access list estimators return this address, but when this address is used as part of the transaction, it does not lower
// the overall gas usage of the transaction, it increases it (might be a bug in node clients that became a consensys).
// should be updated continually as we learn more about the imperfections of 3rd party access_list calculators
#[allow(dead_code)]
fn filter_access_list(access_list: AccessList) -> AccessList {
    access_list
        .into_iter()
        .filter(|item| {
            item.address != H160::from_low_u64_be(1)
                && item
                    .storage_keys
                    .iter()
                    .all(|key| key != &H256::from_low_u64_be(1))
        })
        .collect()
}

pub struct PriorityAccessListEstimating {
    estimators: Vec<Box<dyn AccessListEstimating>>,
}

impl PriorityAccessListEstimating {
    pub fn new(estimators: Vec<Box<dyn AccessListEstimating>>) -> Self {
        Self { estimators }
    }
}

#[async_trait::async_trait]
impl AccessListEstimating for PriorityAccessListEstimating {
    async fn estimate_access_lists(
        &self,
        txs: &[TransactionBuilder<DynTransport>],
    ) -> Result<Vec<Result<AccessList>>> {
        for (i, estimator) in self.estimators.iter().enumerate() {
            match estimator.estimate_access_lists(txs).await {
                Ok(result) => return Ok(result),
                Err(err) => {
                    tracing::warn!("access list estimator {} failed {:?}", i, err);
                }
            }
        }
        Err(anyhow! {"all estimators failed, no access list"})
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ethcontract::{Account, H160};
    use hex_literal::hex;
    use serde_json::json;
    use shared::{transport::create_env_test_transport, Web3};

    fn example_tx() -> TransactionBuilder<DynTransport> {
        let http = create_env_test_transport();
        let web3 = Web3::new(http);
        let account = Account::Local(
            H160::from_slice(&hex!("e92f359e6f05564849afa933ce8f62b8007a1d5d")),
            None,
        );
        let data: Bytes = hex!("13d79a0b00000000000000000000000000000000000000000000000000000000000000800000000000000000000000000000000000000000000000000000000000000100000000000000000000000000000000000000000000000000000000000000018000000000000000000000000000000000000000000000000000000000000005a000000000000000000000000000000000000000000000000000000000000000030000000000000000000000004e3fbd56cd56c3e72c1403e103b45db9da5b9d2b000000000000000000000000990f341946a3fdb507ae7e52d17851b87168017c000000000000000000000000a0b86991c6218b36c1d19d4a2e9eb0ce3606eb48000000000000000000000000000000000000000000000000000000000000000300000000000000000000000000000000000000000000000000000006765a71600000000000000000000000000000000000000000000000000000007347b2e76f0000000000000000000000000000000000000000000000368237ac6c6ad709fe0000000000000000000000000000000000000000000000000000000000000002000000000000000000000000000000000000000000000000000000000000004000000000000000000000000000000000000000000000000000000000000002200000000000000000000000000000000000000000000000000000000000000002000000000000000000000000000000000000000000000000000000000000000000000000000000000000000098e073b579fd483eac8f10d5bd0b32c8c3bbd7e000000000000000000000000000000000000000000000000000000006765a71600000000000000000000000000000000000000000000000363ccb23497d69b5e10000000000000000000000000000000000000000000000000000000061f99a9c487b02c558d729abaf3ecf17881a4181e5bc2446429a0995142297e897b6eb37000000000000000000000000000000000000000000000000000000000e93a6a0000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000006765a716000000000000000000000000000000000000000000000000000000000000001600000000000000000000000000000000000000000000000000000000000000041c5a207f8688e853bdd7402727104da7b4094672dc8672c60840e5d0457e3be85295c881e39e59070ea3b42a79de3c4d6ba7a41d10e1883b2aafc6c77be0518ea1c00000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000100000000000000000000000000000000000000000000000000000000000000020000000000000000000000001aefff55c6b6a53f6b63eab65025446024ebc8e3000000000000000000000000000000000000000000000000de9babded1fb850e00000000000000000000000000000000000000000000000000000001d4734cf00000000000000000000000000000000000000000000000000000000061f99f38487b02c558d729abaf3ecf17881a4181e5bc2446429a0995142297e897b6eb3700000000000000000000000000000000000000000000000001e9db2b61bfd6500000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000de9babded1fb850e0000000000000000000000000000000000000000000000000000000000000160000000000000000000000000000000000000000000000000000000000000004125fa0bacb9c8806fe80910b005e10d9aa5dbb02bd0a66ccdc549d92304625fd95f6e07b36480389e6067894c2bc4ad45617aa11449d5a01b4dcf0a3bf34a33911b00000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000006000000000000000000000000000000000000000000000000000000000000000800000000000000000000000000000000000000000000000000000000000000cc00000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000200000000000000000000000000000000000000000000000000000000000000400000000000000000000000000000000000000000000000000000000000000a40000000000000000000000000def1c0ded9bec7f1a1670819833240f027b25eff000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000600000000000000000000000000000000000000000000000000000000000000968415565b0000000000000000000000000a0b86991c6218b36c1d19d4a2e9eb0ce3606eb480000000000000000000000004e3fbd56cd56c3e72c1403e103b45db9da5b9d2b00000000000000000000000000000000000000000000000000000006765a7160000000000000000000000000000000000000000000000036585ad5a25d351d2a00000000000000000000000000000000000000000000000000000000000000a00000000000000000000000000000000000000000000000000000000000000003000000000000000000000000000000000000000000000000000000000000006000000000000000000000000000000000000000000000000000000000000003c0000000000000000000000000000000000000000000000000000000000000070000000000000000000000000000000000000000000000000000000000000000150000000000000000000000000000000000000000000000000000000000000040000000000000000000000000000000000000000000000000000000000000030000000000000000000000000000000000000000000000000000000000000000200000000000000000000000000000000000000000000000000000000000000000000000000000000000000000a0b86991c6218b36c1d19d4a2e9eb0ce3606eb48000000000000000000000000c02aaa39b223fe8d0a0e5c4f27ead9083c756cc2000000000000000000000000000000000000000000000000000000000000012000000000000000000000000000000000000000000000000000000000000002c000000000000000000000000000000000000000000000000000000000000002c000000000000000000000000000000000000000000000000000000000000002a000000000000000000000000000000000000000000000000000000006765a716000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000001000000000000000000000000000000000000000000000000000000000000002000000000000000000000000000000012556e697377617056330000000000000000000000000000000000000000000000000000000000000000000006765a71600000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000008000000000000000000000000000000000000000000000000000000000000000a0000000000000000000000000e592427a0aece92de3edee1f18e0157c058615640000000000000000000000000000000000000000000000000000000000000040000000000000000000000000000000000000000000000000000000000000002ba0b86991c6218b36c1d19d4a2e9eb0ce3606eb480001f4c02aaa39b223fe8d0a0e5c4f27ead9083c756cc2000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000100000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000015000000000000000000000000000000000000000000000000000000000000004000000000000000000000000000000000000000000000000000000000000002e000000000000000000000000000000000000000000000000000000000000000200000000000000000000000000000000000000000000000000000000000000000000000000000000000000000c02aaa39b223fe8d0a0e5c4f27ead9083c756cc20000000000000000000000004e3fbd56cd56c3e72c1403e103b45db9da5b9d2b000000000000000000000000000000000000000000000000000000000000012000000000000000000000000000000000000000000000000000000000000002a000000000000000000000000000000000000000000000000000000000000002a00000000000000000000000000000000000000000000000000000000000000280ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff0000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000100000000000000000000000000000000000000000000000000000000000000200000000000000000000000000000000143757276650000000000000000000000ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff000000000000000000000000000000000000000000000036585ad5a25d351d2900000000000000000000000000000000000000000000000000000000000000800000000000000000000000000000000000000000000000000000000000000080000000000000000000000000b576491f1e6e5e62f1d8f26062ee822b40b0e0d465b2489b0000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000001000000000000000000000000000000000000000000000000000000000000000100000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000007000000000000000000000000000000000000000000000000000000000000004000000000000000000000000000000000000000000000000000000000000001000000000000000000000000000000000000000000000000000000000000000020000000000000000000000000000000000000000000000000000000000000004000000000000000000000000000000000000000000000000000000000000000c00000000000000000000000000000000000000000000000000000000000000003000000000000000000000000a0b86991c6218b36c1d19d4a2e9eb0ce3606eb48000000000000000000000000c02aaa39b223fe8d0a0e5c4f27ead9083c756cc2000000000000000000000000eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee0000000000000000000000000000000000000000000000000000000000000000869584cd0000000000000000000000009008d19f58aabd9ed0d60971565aa8510560ab410000000000000000000000000000000000000000000000649e79ae6861f99856000000000000000000000000000000000000000000000000000000000000000000000000def1c0ded9bec7f1a1670819833240f027b25eff0000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000006000000000000000000000000000000000000000000000000000000000000001486af479b20000000000000000000000000000000000000000000000000000000000000080000000000000000000000000000000000000000000000000de9babded1fb850e00000000000000000000000000000000000000000000000000000001d561592a00000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000042990f341946a3fdb507ae7e52d17851b87168017c000bb8c02aaa39b223fe8d0a0e5c4f27ead9083c756cc20001f4a0b86991c6218b36c1d19d4a2e9eb0ce3606eb48000000000000000000000000000000000000000000000000000000000000869584cd0000000000000000000000009008d19f58aabd9ed0d60971565aa8510560ab410000000000000000000000000000000000000000000000a5b49e4eb461f998560000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000").into();

        TransactionBuilder::new(web3)
            .from(account)
            .to(H160::from_slice(&hex!(
                "9008d19f58aabd9ed0d60971565aa8510560ab41"
            )))
            .data(data)
    }

    #[tokio::test]
    #[ignore]
    async fn tenderly_estimate_access_lists() {
        let tenderly_api = TenderlyApi::new(
            // http://api.tenderly.co/api/v1/account/<USER_NAME>/project/<PROJECT_NAME>/simulate
            Url::parse(&std::env::var("TENDERLY_URL").unwrap()).unwrap(),
            Client::new(),
            &std::env::var("TENDERLY_API_KEY").unwrap(),
            "1".to_string(),
        )
        .unwrap();

        let tx = example_tx();
        let access_lists = tenderly_api.estimate_access_lists(&[tx]).await.unwrap();
        dbg!(access_lists);
    }

    #[tokio::test]
    #[ignore]
    async fn node_estimate_access_lists() {
        let http = create_env_test_transport();
        let web3 = Web3::new(http);
        let node_api = NodeApi::new(web3);

        let tx = example_tx();

        let access_lists = node_api.estimate_access_lists(&[tx]).await.unwrap();
        dbg!(access_lists);
    }

    #[tokio::test]
    #[ignore]
    async fn node_estimate_multiple_access_lists() {
        let http = create_env_test_transport();
        let web3 = Web3::new(http);
        let node_api = NodeApi::new(web3.clone());

        let tx = example_tx();
        let tx2 = TransactionBuilder::new(web3); //empty transaction
        let tx3 = example_tx();

        let access_lists = node_api
            .estimate_access_lists(&[tx, tx2, tx3])
            .await
            .unwrap();
        dbg!(access_lists);
    }

    #[test]
    fn serialize_deserialize_request() {
        let request = TenderlyRequest {
            network_id: "1".to_string(),
            block_number: 14122310,
            from: H160::from_slice(&hex!("e92f359e6f05564849afa933ce8f62b8007a1d5d")),
            input: hex!("13d79a0b00000000000000000000000000000000000000000000").into(),
            to: H160::from_slice(&hex!("9008d19f58aabd9ed0d60971565aa8510560ab41")),
            generate_access_list: true,
        };

        let json = json!({
            "network_id": "1",
            "block_number": 14122310,
            "from": "0xe92f359e6f05564849afa933ce8f62b8007a1d5d",
            "input": "0x13d79a0b00000000000000000000000000000000000000000000",
            "to": "0x9008d19f58aabd9ed0d60971565aa8510560ab41",
            "generate_access_list": true
        });

        assert_eq!(serde_json::to_value(&request).unwrap(), json);
        assert_eq!(
            serde_json::from_value::<TenderlyRequest>(json).unwrap(),
            request
        );
    }

    #[test]
    fn filter_access_list_node() {
        let access_list = json!(
            [
            {
                "address": "0x9008d19f58aabd9ed0d60971565aa8510560ab41",
                "storageKeys": [
                    "0x0000000000000000000000000000000000000000000000000000000000000001",
                ],
            },
            {
                "address": "0x2c4c28ddbdac9c5e7055b4c863b72ea0149d8afe",
                "storageKeys": [
                    "0x360894a13ba1a3210667c828492db98dca3e2076cc3735a920a3ca505d382bbc",
                    "0x3e0a6b9ca93e33d18d2a2214f9ba022e0362fbadbdf27cd46f31629229baa68b",
                ],
            },
            {
                "address": "0x9e7ae8bdba9aa346739792d219a808884996db67",
                "storageKeys": [],
            },
        ]
        );
        let access_list = helpers::decode::<AccessList>(access_list).unwrap();
        let access_list = filter_access_list(access_list);

        let expected = json!(
            [
            {
                "address": "0x2c4c28ddbdac9c5e7055b4c863b72ea0149d8afe",
                "storageKeys": [
                    "0x360894a13ba1a3210667c828492db98dca3e2076cc3735a920a3ca505d382bbc",
                    "0x3e0a6b9ca93e33d18d2a2214f9ba022e0362fbadbdf27cd46f31629229baa68b",
                ],
            },
            {
                "address": "0x9e7ae8bdba9aa346739792d219a808884996db67",
                "storageKeys": [],
            },
        ]
        );
        let expected = helpers::decode::<AccessList>(expected).unwrap();

        assert_eq!(access_list, expected);
    }

    #[test]
    fn filter_access_list_tenderly() {
        let access_list = json!(
            [
            {
                "address": "0x0000000000000000000000000000000000000001",
                "storageKeys": [],
            },
            {
                "address": "0x2c4c28ddbdac9c5e7055b4c863b72ea0149d8afe",
                "storageKeys": [
                    "0x360894a13ba1a3210667c828492db98dca3e2076cc3735a920a3ca505d382bbc",
                    "0x3e0a6b9ca93e33d18d2a2214f9ba022e0362fbadbdf27cd46f31629229baa68b",
                ],
            },
            {
                "address": "0x9e7ae8bdba9aa346739792d219a808884996db67",
                "storageKeys": [],
            },
        ]
        );
        let access_list = helpers::decode::<AccessList>(access_list).unwrap();
        let access_list = filter_access_list(access_list);

        let expected = json!(
            [
            {
                "address": "0x2c4c28ddbdac9c5e7055b4c863b72ea0149d8afe",
                "storageKeys": [
                    "0x360894a13ba1a3210667c828492db98dca3e2076cc3735a920a3ca505d382bbc",
                    "0x3e0a6b9ca93e33d18d2a2214f9ba022e0362fbadbdf27cd46f31629229baa68b",
                ],
            },
            {
                "address": "0x9e7ae8bdba9aa346739792d219a808884996db67",
                "storageKeys": [],
            },
        ]
        );
        let expected = helpers::decode::<AccessList>(expected).unwrap();

        assert_eq!(access_list, expected);
    }
}
