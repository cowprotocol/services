//! A block builder that accepts `eth_sendBundle` like a real one and forwards
//! the bundled transactions to the test node.

use {
    alloy::{
        providers::{Provider, ext::verify_flashbots_signature},
        rpc::types::mev::EthSendBundle,
    },
    axum::{body::Bytes, extract::State, http::HeaderMap, response::Json},
    ethrpc::{AlloyProvider, Web3},
    serde_json::{Value, json},
    std::sync::{Arc, Mutex},
    tokio::net::TcpListener,
};

pub struct Builder {
    listener: TcpListener,
}

impl Builder {
    /// Reserves the address of the builder. The test node only exists once
    /// the test is set up, so the builder starts serving later.
    pub async fn bind() -> Self {
        Self {
            listener: TcpListener::bind("0.0.0.0:0").await.unwrap(),
        }
    }

    pub fn url(&self) -> String {
        format!("http://{}", self.listener.local_addr().unwrap())
    }

    /// Starts accepting bundles and forwards their transactions to `node`.
    pub fn serve(self, node: &Web3) -> Requests {
        let requests = Requests::default();
        let state = Served {
            node: node.provider.clone(),
            requests: requests.clone(),
        };
        let app = axum::Router::new()
            .route("/", axum::routing::post(handle))
            .with_state(state);
        tokio::spawn(async move { axum::serve(self.listener, app).await.unwrap() });
        requests
    }
}

/// The requests a builder received.
#[derive(Clone, Default)]
pub struct Requests(Arc<Mutex<Vec<Value>>>);

impl Requests {
    pub fn is_empty(&self) -> bool {
        self.0.lock().unwrap().is_empty()
    }

    /// The blocks targeted by the bundles, in ascending order.
    pub fn bundle_blocks(&self) -> Vec<u64> {
        let mut blocks: Vec<_> = self
            .0
            .lock()
            .unwrap()
            .iter()
            .filter(|request| request["method"] == "eth_sendBundle")
            .map(|request| {
                request["params"][0]["blockNumber"]
                    .as_str()
                    .unwrap()
                    .to_owned()
            })
            .map(|block| u64::from_str_radix(block.trim_start_matches("0x"), 16).unwrap())
            .collect();
        blocks.sort();
        blocks
    }
}

#[derive(Clone)]
struct Served {
    node: AlloyProvider,
    requests: Requests,
}

async fn handle(State(state): State<Served>, headers: HeaderMap, body: Bytes) -> Json<Value> {
    let request: Value = serde_json::from_slice(&body).unwrap();
    state.requests.0.lock().unwrap().push(request.clone());
    // Like real builders, only take bundles.
    if request["method"] != "eth_sendBundle" {
        return Json(json!({
            "jsonrpc": "2.0",
            "id": request["id"],
            "error": { "code": -32601, "message": "Method not found" },
        }));
    }

    let signature = headers
        .get("x-flashbots-signature")
        .expect("missing X-Flashbots-Signature")
        .to_str()
        .unwrap();
    verify_flashbots_signature(signature, &body).expect("invalid X-Flashbots-Signature");
    let bundle: EthSendBundle = serde_json::from_value(request["params"][0].clone()).unwrap();
    assert!(
        bundle.reverting_tx_hashes.is_empty(),
        "settlements must not be allowed to revert"
    );

    for tx in &bundle.txs {
        // Every block until the deadline gets the same tx, so the node
        // rejects all copies but the first.
        let _ = state.node.send_raw_transaction(tx).await;
    }

    Json(json!({
        "jsonrpc": "2.0",
        "id": request["id"],
        "result": { "bundleHash": bundle.bundle_hash() },
    }))
}
