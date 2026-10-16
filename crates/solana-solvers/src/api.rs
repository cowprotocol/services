//! HTTP API for the solver engine.
//!
//! Serves the `/solve` and `/cosign` contracts the driver calls.

use {
    crate::{
        dex::{self, Dex},
        domain::solver::Solver,
        dto::{auction::Auction, cosign},
        extract::LoggingJson,
    },
    axum::{
        Json,
        Router,
        extract::State,
        http::StatusCode,
        routing::{get, post},
    },
    serde_json::{Value, json},
    std::{future::Future, net::SocketAddr, sync::Arc},
    tower_http::limit::RequestBodyLimitLayer,
};

const REQUEST_BODY_LIMIT: usize = 10 * 1024 * 1024;

pub struct Api {
    pub addr: SocketAddr,
    pub solver: Arc<Solver<Dex>>,
}

impl Api {
    /// Bind to the configured address, returning the listener and the actual
    /// bound address (which differs from `addr` when binding to port 0).
    pub async fn bind(&self) -> std::io::Result<(tokio::net::TcpListener, SocketAddr)> {
        let listener = tokio::net::TcpListener::bind(self.addr).await?;
        let local_addr = listener.local_addr()?;
        tracing::info!(addr = %local_addr, "solana-solvers listening");
        Ok((listener, local_addr))
    }

    /// Serve the API on the given listener until `shutdown` resolves.
    pub async fn serve(
        self,
        listener: tokio::net::TcpListener,
        shutdown: impl Future<Output = ()> + Send + 'static,
    ) -> std::io::Result<()> {
        let app = Router::new()
            .route("/healthz", get(healthz))
            .route("/solve", post(solve))
            .route("/cosign", post(cosign))
            .with_state(self.solver)
            .layer(RequestBodyLimitLayer::new(REQUEST_BODY_LIMIT))
            .layer(axum::extract::DefaultBodyLimit::disable());

        axum::serve(listener, app)
            .with_graceful_shutdown(shutdown)
            .await
    }
}

async fn healthz() -> &'static str {
    "ok"
}

/// Quote the auction's orders and return the single-order solutions.
async fn solve(
    State(solver): State<Arc<Solver<Dex>>>,
    LoggingJson(auction): LoggingJson<Auction>,
) -> Json<Value> {
    let solutions = solver.solve(&auction).await;
    Json(json!({ "solutions": solutions }))
}

/// Add the signatures the swap's other signers owe on the driver-signed
/// transaction.
async fn cosign(
    State(solver): State<Arc<Solver<Dex>>>,
    Json(request): Json<cosign::Transaction>,
) -> Result<Json<cosign::Transaction>, (StatusCode, String)> {
    let transaction = request
        .into_transaction()
        .map_err(|err| (StatusCode::BAD_REQUEST, err))?;
    let cosigned = solver.cosign(&transaction).await.map_err(|err| {
        tracing::warn!(%err, "co-signing failed");
        let status = match err {
            dex::Error::CosignNotSupported => StatusCode::NOT_IMPLEMENTED,
            _ => StatusCode::BAD_GATEWAY,
        };
        (status, err.to_string())
    })?;
    Ok(Json(cosign::Transaction::new(&cosigned)))
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        crate::{
            config::{BitgetConfig, Fee},
            dex::bitget::Bitget,
        },
        base64::prelude::*,
        configs::rate_limit::Strategy,
        solana_sdk::{
            hash::Hash,
            instruction::{AccountMeta, Instruction},
            message::{VersionedMessage, v0},
            pubkey::Pubkey,
            signature::Signature,
            signer::{Signer as _, keypair::Keypair},
            transaction::VersionedTransaction,
        },
        std::{num::NonZeroUsize, time::Duration},
    };

    /// Stands in for Bitget's `/signBgwSolTx`: decodes the base58
    /// transaction, signs `authority`'s slot, and answers in Bitget's
    /// envelope.
    async fn spawn_bitget(authority: Arc<Keypair>) -> SocketAddr {
        let app = Router::new().route(
            "/signBgwSolTx",
            post(move |Json(request): Json<Value>| async move {
                assert_eq!(request["chain"], "sol");
                let bytes = bs58::decode(request["transaction"].as_str().unwrap())
                    .into_vec()
                    .unwrap();
                let mut transaction: VersionedTransaction = bincode::deserialize(&bytes).unwrap();
                let index = transaction
                    .message
                    .static_account_keys()
                    .iter()
                    .position(|key| *key == authority.pubkey())
                    .unwrap();
                transaction.signatures[index] =
                    authority.sign_message(&transaction.message.serialize());
                let encoded = bs58::encode(bincode::serialize(&transaction).unwrap()).into_string();
                Json(json!({"status": 0, "error_code": 0, "data": {"transaction": encoded}}))
            }),
        );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        addr
    }

    /// `/cosign` hands the driver-signed transaction to Bitget and returns it
    /// with Bitget's signature added and nothing else changed.
    #[tokio::test]
    async fn cosigns_through_bitget() {
        let authority = Arc::new(Keypair::new());
        let bitget = Bitget::new(&BitgetConfig {
            endpoint: format!("http://{}/", spawn_bitget(Arc::clone(&authority)).await)
                .parse()
                .unwrap(),
            api_key: "key".into(),
            api_secret: "secret".into(),
            slippage_bps: 20,
            max_accounts: 48,
            fee: Fee::Exempt,
        })
        .unwrap();
        let solver = Solver::new(
            Dex::Bitget(bitget),
            NonZeroUsize::new(1).unwrap(),
            Strategy::try_new(2.0, Duration::from_secs(1), Duration::from_secs(8)).unwrap(),
        );
        let api = Api {
            addr: "127.0.0.1:0".parse().unwrap(),
            solver: Arc::new(solver),
        };
        let (listener, addr) = api.bind().await.unwrap();
        tokio::spawn(api.serve(listener, std::future::pending()));

        let taker = Keypair::new();
        let route = Instruction::new_with_bytes(
            Pubkey::new_unique(),
            &[],
            vec![AccountMeta::new_readonly(authority.pubkey(), true)],
        );
        let message = VersionedMessage::V0(
            v0::Message::try_compile(&taker.pubkey(), &[route], &[], Hash::new_unique()).unwrap(),
        );
        let transaction = VersionedTransaction {
            signatures: vec![
                taker.sign_message(&message.serialize()),
                Signature::default(),
            ],
            message,
        };

        let encoded = BASE64_STANDARD.encode(bincode::serialize(&transaction).unwrap());
        let response = reqwest::Client::new()
            .post(format!("http://{addr}/cosign"))
            .json(&json!({ "transaction": encoded }))
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), reqwest::StatusCode::OK);
        let body = response.json::<Value>().await.unwrap();
        let bytes = BASE64_STANDARD
            .decode(body["transaction"].as_str().unwrap())
            .unwrap();
        let cosigned: VersionedTransaction = bincode::deserialize(&bytes).unwrap();
        assert_eq!(cosigned.message, transaction.message);
        assert_eq!(cosigned.verify_with_results(), [true, true]);
    }
}
