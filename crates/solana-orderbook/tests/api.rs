//! Integration tests for the HTTP API server.

use {
    base64::Engine,
    cow_solana_rpc::{Mocks, RpcRequest, SolanaRPC},
    database::{byte_array::ByteArray, solana::OrderKind},
    solana_orderbook::infra::{api::Api, db, quoter::Quoter},
    solana_sdk::signer::Signer,
    sqlx::PgPool,
    std::{net::SocketAddr, time::Duration},
    tokio_util::sync::CancellationToken,
};

fn mock_api() -> Api {
    Api {
        addr: "0.0.0.0:0".parse().unwrap(),
        // A lazy pool at a dead endpoint keeps these tests database-free: a
        // quote insert degrades to an id-less answer, nothing else queries.
        pool: PgPool::connect_lazy("postgresql://127.0.0.1:1/").unwrap(),
        quoter: dead_quoter(),
        validation: Default::default(),
        quote_expiry: Duration::from_secs(60),
        sponsoring: None,
    }
}

/// A quoter pointing at a dead endpoint: every quote attempt fails.
fn dead_quoter() -> Quoter {
    Quoter::new(
        vec!["http://127.0.0.1:1/".parse().unwrap()],
        Duration::from_secs(1),
    )
}

/// Spawn the API server on an ephemeral port and return its bound address.
async fn spawn_server() -> SocketAddr {
    spawn_server_with(dead_quoter()).await
}

/// Spawn the API server with the given quoter.
async fn spawn_server_with(quoter: Quoter) -> SocketAddr {
    let api = Api {
        quoter,
        ..mock_api()
    };
    let (listener, addr) = api.bind().await.unwrap();
    // A token that is never cancelled keeps the server alive for the test.
    let shutdown = CancellationToken::new();
    tokio::spawn(async move { api.serve(listener, shutdown).await.unwrap() });
    addr
}

/// A tiny axum server that answers `/quote` with a fixed response. It stands
/// in for the driver.
async fn spawn_mock_driver(response: serde_json::Value) -> SocketAddr {
    let app = axum::Router::new().route(
        "/quote",
        axum::routing::post(move || {
            let response = response.clone();
            async move { axum::Json(response) }
        }),
    );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    addr
}

#[tokio::test]
async fn healthz_returns_200() {
    let addr = spawn_server().await;
    let response = reqwest::Client::new()
        .get(format!("http://{addr}/healthz"))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), reqwest::StatusCode::OK);
}

#[tokio::test]
async fn shuts_down_cleanly_on_signal() {
    let api = mock_api();
    let (listener, addr) = api.bind().await.unwrap();
    let shutdown_token = CancellationToken::new();
    let serve = api.serve(listener, shutdown_token.clone());
    let handle = tokio::spawn(async move { serve.await.unwrap() });

    // The server is up and serving requests.
    let response = reqwest::Client::new()
        .get(format!("http://{addr}/healthz"))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), reqwest::StatusCode::OK);

    // Trigger graceful shutdown and assert the serve future completes cleanly.
    shutdown_token.cancel();
    handle.await.unwrap();
}

/// A malformed quote body is rejected in the API's error shape, not axum's
/// plain-text default.
#[tokio::test]
async fn malformed_quote_body_keeps_the_error_shape() {
    let addr = spawn_server().await;
    let response = reqwest::Client::new()
        .post(format!("http://{addr}/api/v1/quote"))
        .json(&serde_json::json!({"sellToken": "not-a-pubkey"}))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), reqwest::StatusCode::BAD_REQUEST);
    assert_eq!(
        response.json::<serde_json::Value>().await.unwrap(),
        serde_json::json!({
            "errorType": "InvalidRequestBody",
            "description": "The request body could not be parsed."
        })
    );
}

/// A quote body with the given validity fields, otherwise well formed.
fn quote_body(validity: serde_json::Value) -> serde_json::Value {
    let mut body = serde_json::json!({
        "from": "9VXC6LH9eXMBpXLQnxMYAGkjs59Zon2ACciJwQ6iMzNB",
        "sellToken": "So11111111111111111111111111111111111111112",
        "buyToken": "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v",
        "kind": "sell",
        "sellAmountBeforeFee": "10000000"
    });
    body.as_object_mut()
        .unwrap()
        .extend(validity.as_object().unwrap().clone());
    body
}

async fn post_quote(addr: SocketAddr, body: serde_json::Value) -> (reqwest::StatusCode, String) {
    let response = reqwest::Client::new()
        .post(format!("http://{addr}/api/v1/quote"))
        .json(&body)
        .send()
        .await
        .unwrap();
    let status = response.status();
    let json: serde_json::Value = response.json().await.unwrap();
    (
        status,
        json["errorType"].as_str().unwrap_or_default().to_owned(),
    )
}

/// Validation runs before any driver is asked, so every rejection here
/// answers 400 without reaching the dead quoter.
#[tokio::test]
async fn quote_validation_rejects_bad_orders() {
    let addr = spawn_server().await;
    let mut same_tokens = quote_body(serde_json::json!({"validFor": 1800}));
    same_tokens["buyToken"] = same_tokens["sellToken"].clone();
    let mut zero_amount = quote_body(serde_json::json!({"validFor": 1800}));
    zero_amount["sellAmountBeforeFee"] = serde_json::json!("0");

    for (body, expected) in [
        (
            quote_body(serde_json::json!({"validFor": 10})),
            "InsufficientValidTo",
        ),
        (
            quote_body(serde_json::json!({"validFor": 4 * 60 * 60})),
            "ExcessiveValidTo",
        ),
        (same_tokens, "SameBuyAndSellToken"),
        (zero_amount, "ZeroAmount"),
    ] {
        let (status, kind) = post_quote(addr, body).await;
        assert_eq!(
            (status, kind.as_str()),
            (reqwest::StatusCode::BAD_REQUEST, expected)
        );
    }
}

/// The full happy path: the driver's amounts come back in the EVM response
/// shape with the request's own fields echoed.
#[tokio::test]
async fn quote_answers_in_the_evm_shape() {
    let driver = spawn_mock_driver(serde_json::json!({
        "sellAmount": "10000000",
        "buyAmount": "1234567",
        "solver": "9VXC6LH9eXMBpXLQnxMYAGkjs59Zon2ACciJwQ6iMzNB",
    }))
    .await;
    let addr = spawn_server_with(Quoter::new(
        vec![format!("http://{driver}/").parse().unwrap()],
        Duration::from_secs(1),
    ))
    .await;

    let valid_to = chrono::Utc::now().timestamp() + 600;
    let mut body = quote_body(serde_json::json!({"validTo": valid_to}));
    body["receiver"] = serde_json::json!("EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v");
    body["appData"] = serde_json::json!(format!("0x{}", "11".repeat(32)));

    let response = reqwest::Client::new()
        .post(format!("http://{addr}/api/v1/quote"))
        .json(&body)
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), reqwest::StatusCode::OK);

    let json: serde_json::Value = response.json().await.unwrap();
    assert_eq!(
        json,
        serde_json::json!({
            "quote": {
                "sellToken": body["sellToken"],
                "buyToken": body["buyToken"],
                "receiver": body["receiver"],
                "sellAmount": "10000000",
                "buyAmount": "1234567",
                "validTo": valid_to,
                "appData": body["appData"],
                "feeAmount": "0",
                "kind": "sell",
                "partiallyFillable": false,
            },
            "from": body["from"],
            "expiration": json["expiration"],
            "id": null,
            "verified": false,
        })
    );
    // The amounts are honored for about a minute from now.
    let expiration: chrono::DateTime<chrono::Utc> =
        json["expiration"].as_str().unwrap().parse().unwrap();
    let honored_for = (expiration - chrono::Utc::now()).num_seconds();
    assert!(
        (50..=60).contains(&honored_for),
        "expiration {honored_for}s away"
    );
}

/// With several drivers configured, the best answer wins: the largest buy
/// amount for a sell order.
#[tokio::test]
async fn quote_picks_the_best_driver_answer() {
    let worse = spawn_mock_driver(serde_json::json!({
        "sellAmount": "10000000",
        "buyAmount": "1500000",
        "solver": "9VXC6LH9eXMBpXLQnxMYAGkjs59Zon2ACciJwQ6iMzNB",
    }))
    .await;
    let better = spawn_mock_driver(serde_json::json!({
        "sellAmount": "10000000",
        "buyAmount": "2000000",
        "solver": "9VXC6LH9eXMBpXLQnxMYAGkjs59Zon2ACciJwQ6iMzNB",
    }))
    .await;
    let addr = spawn_server_with(Quoter::new(
        vec![
            format!("http://{worse}/").parse().unwrap(),
            format!("http://{better}/").parse().unwrap(),
            "http://127.0.0.1:1/".parse().unwrap(),
        ],
        Duration::from_secs(1),
    ))
    .await;

    let response = reqwest::Client::new()
        .post(format!("http://{addr}/api/v1/quote"))
        .json(&quote_body(serde_json::json!({"validFor": 1800})))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), reqwest::StatusCode::OK);
    let json: serde_json::Value = response.json().await.unwrap();
    assert_eq!(json["quote"]["buyAmount"], "2000000");
}

/// Every driver failure reads as no liquidity, mirroring the EVM mapping of
/// estimator errors.
#[tokio::test]
async fn quote_without_a_route_is_no_liquidity() {
    let addr = spawn_server().await;
    let (status, kind) = post_quote(addr, quote_body(serde_json::json!({"validFor": 1800}))).await;
    assert_eq!(
        (status, kind.as_str()),
        (reqwest::StatusCode::NOT_FOUND, "NoLiquidity")
    );
}

/// The answered quote lands in `solana.quotes` and its id comes back.
#[tokio::test]
#[ignore = "needs the solana.* schema applied to the local database"]
async fn solana_db_quote_is_persisted() {
    let pool = PgPool::connect("postgresql://").await.unwrap();
    sqlx::query("TRUNCATE solana.quotes")
        .execute(&pool)
        .await
        .unwrap();
    let solver = solana_sdk::pubkey::Pubkey::new_unique();
    let driver = spawn_mock_driver(serde_json::json!({
        "sellAmount": "10000000",
        "buyAmount": "1234567",
        "solver": solver.to_string(),
    }))
    .await;
    let api = Api {
        pool: pool.clone(),
        quoter: Quoter::new(
            vec![format!("http://{driver}/").parse().unwrap()],
            Duration::from_secs(1),
        ),
        ..mock_api()
    };
    let (listener, addr) = api.bind().await.unwrap();
    let shutdown = CancellationToken::new();
    tokio::spawn(async move { api.serve(listener, shutdown).await.unwrap() });

    let response = reqwest::Client::new()
        .post(format!("http://{addr}/api/v1/quote"))
        .json(&quote_body(serde_json::json!({"validFor": 1800})))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), reqwest::StatusCode::OK);
    let json: serde_json::Value = response.json().await.unwrap();
    let id = json["id"].as_i64().unwrap();

    type Row = (
        i64,
        Vec<u8>,
        Vec<u8>,
        String,
        String,
        String,
        Vec<u8>,
        chrono::DateTime<chrono::Utc>,
    );
    let row: Row = sqlx::query_as(
        "SELECT id, sell_token, buy_token, sell_amount::text, buy_amount::text, kind::text, \
         solver, expiration_timestamp FROM solana.quotes",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    let sell: solana_sdk::pubkey::Pubkey = "So11111111111111111111111111111111111111112"
        .parse()
        .unwrap();
    let buy: solana_sdk::pubkey::Pubkey = "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v"
        .parse()
        .unwrap();
    assert_eq!(row.0, id);
    assert_eq!(row.1, sell.to_bytes());
    assert_eq!(row.2, buy.to_bytes());
    assert_eq!(row.3, "10000000");
    assert_eq!(row.4, "1234567");
    assert_eq!(row.5, "sell");
    assert_eq!(row.6, solver.to_bytes());
    // The stored expiry is the answered one, up to timestamptz rounding.
    let answered: chrono::DateTime<chrono::Utc> =
        json["expiration"].as_str().unwrap().parse().unwrap();
    assert!((row.7 - answered).num_milliseconds().abs() <= 1);
}

/// Parameter validation of the trades endpoint short-circuits before any
/// database access.
#[tokio::test]
async fn trades_rejects_an_invalid_limit() {
    let addr = spawn_server().await;
    let uid = "11".repeat(32);
    for limit in ["0", "1001"] {
        let response = reqwest::Client::new()
            .get(format!(
                "http://{addr}/api/v2/trades?orderUid={uid}&limit={limit}"
            ))
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), reqwest::StatusCode::BAD_REQUEST);
        let json: serde_json::Value = response.json().await.unwrap();
        assert_eq!(json["errorType"], "InvalidLimit");
    }
}

/// Parameter validation of the account orders endpoint short-circuits before
/// any database access.
#[tokio::test]
async fn account_orders_rejects_bad_parameters() {
    let addr = spawn_server().await;
    let client = reqwest::Client::new();

    let response = client
        .get(format!("http://{addr}/api/v1/account/not-a-pubkey/orders"))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), reqwest::StatusCode::BAD_REQUEST);
    let json: serde_json::Value = response.json().await.unwrap();
    assert_eq!(json["errorType"], "InvalidOwner");

    let response = client
        .get(format!(
            "http://{addr}/api/v1/account/9VXC6LH9eXMBpXLQnxMYAGkjs59Zon2ACciJwQ6iMzNB/orders?limit=0"
        ))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), reqwest::StatusCode::BAD_REQUEST);
    let json: serde_json::Value = response.json().await.unwrap();
    assert_eq!(json["errorType"], "LIMIT_OUT_OF_BOUNDS");
}

/// A sponsored-placement server: a fixed funder pubkey, the interface's
/// default settlement program, and a mock RPC answering the blockhash and
/// height probes.
async fn spawn_sponsored_server(
    pool: PgPool,
    funder: solana_sdk::pubkey::Pubkey,
    blockhash_valid: bool,
) -> SocketAddr {
    let mocks = Mocks::from([
        (
            RpcRequest::IsBlockhashValid,
            serde_json::json!({
                "context": { "slot": 1u64, "apiVersion": "2.0.0" },
                "value": blockhash_valid,
            }),
        ),
        (RpcRequest::GetBlockHeight, serde_json::json!(100u64)),
    ]);
    let api = Api {
        pool,
        sponsoring: Some(solana_orderbook::infra::api::Sponsoring {
            funder,
            settlement_program: cow_settlement_interface::id(),
            rpc: SolanaRPC::new_mock_with_mocks(mocks),
        }),
        ..mock_api()
    };
    let (listener, addr) = api.bind().await.unwrap();
    let shutdown = CancellationToken::new();
    tokio::spawn(async move { api.serve(listener, shutdown).await.unwrap() });
    addr
}

/// The order intent a sponsored transaction carries. `native` sells SOL
/// through the wSOL mint with real associated token accounts, so preparation
/// steps can accompany the transaction.
fn sponsored_intent(
    owner: solana_sdk::pubkey::Pubkey,
    native: bool,
) -> cow_settlement_interface::data::intent::OrderIntent {
    let buy_mint = solana_sdk::pubkey::Pubkey::new_from_array([0x55; 32]);
    let (sell_mint, sell_token_account, buy_token_account) = if native {
        let sell_mint = spl_token_interface::native_mint::ID;
        (sell_mint, ata(owner, sell_mint), ata(owner, buy_mint))
    } else {
        (
            solana_sdk::pubkey::Pubkey::new_from_array([0x66; 32]),
            solana_sdk::pubkey::Pubkey::new_from_array([0x33; 32]),
            ata(owner, buy_mint),
        )
    };
    cow_settlement_interface::data::intent::OrderIntent {
        owner,
        buy_token_account,
        sell_token_account,
        buy_mint,
        sell_mint,
        sell_amount: 1_000,
        buy_amount: 2_000,
        valid_to: u32::MAX,
        flags: cow_settlement_interface::data::intent::Flags {
            created_on_chain: true,
            kind: cow_settlement_interface::data::intent::OrderKind::Sell,
            partially_fillable: false,
        },
        app_data: [0x44; 32],
    }
}

/// The owner's associated token account for `mint` under the SPL Token
/// program.
fn ata(
    owner: solana_sdk::pubkey::Pubkey,
    mint: solana_sdk::pubkey::Pubkey,
) -> solana_sdk::pubkey::Pubkey {
    spl_associated_token_account_interface::address::get_associated_token_address_with_program_id(
        &owner,
        &mint,
        &spl_token_interface::ID,
    )
}

/// The settlement state PDA delegations must target.
fn state_pda() -> solana_sdk::pubkey::Pubkey {
    cow_settlement_interface::pda::state::find_state_pda(&cow_settlement_interface::id()).0
}

/// The full whitelisted preparation prefix for a native-sell intent: create
/// the wSOL account, fund it, sync it, delegate it, create the buy account.
fn full_preparations(
    funder: solana_sdk::pubkey::Pubkey,
    owner: solana_sdk::pubkey::Pubkey,
    intent: &cow_settlement_interface::data::intent::OrderIntent,
) -> Vec<solana_sdk::instruction::Instruction> {
    vec![
        spl_associated_token_account_interface::instruction::create_associated_token_account_idempotent(
            &funder,
            &owner,
            &intent.sell_mint,
            &spl_token_interface::ID,
        ),
        solana_system_interface::instruction::transfer(&owner, &intent.sell_token_account, 1_000),
        spl_token_interface::instruction::sync_native(
            &spl_token_interface::ID,
            &intent.sell_token_account,
        )
        .unwrap(),
        spl_token_interface::instruction::approve(
            &spl_token_interface::ID,
            &intent.sell_token_account,
            &state_pda(),
            &owner,
            &[],
            1_000,
        )
        .unwrap(),
        spl_associated_token_account_interface::instruction::create_associated_token_account(
            &funder,
            &owner,
            &intent.buy_mint,
            &spl_token_interface::ID,
        ),
    ]
}

/// A partially signed sponsored creation transaction: the given preparation
/// instructions in front of `CreateOrder`, the owner signed, the fee payer
/// slot (the funder) left as a placeholder.
fn creation_tx(
    funder: solana_sdk::pubkey::Pubkey,
    owner: &solana_sdk::signer::keypair::Keypair,
    intent: &cow_settlement_interface::data::intent::OrderIntent,
    preparations: Vec<solana_sdk::instruction::Instruction>,
    sign: bool,
) -> String {
    let mut instructions = preparations;
    instructions.push(
        cow_settlement_client::instruction::CreateOrder {
            program_id: cow_settlement_interface::id(),
            owner: owner.pubkey(),
            created_by: funder,
            intent,
        }
        .into(),
    );
    let message = solana_sdk::message::Message::new_with_blockhash(
        &instructions,
        Some(&funder),
        &solana_sdk::hash::Hash::new_unique(),
    );
    let signers = usize::from(message.header.num_required_signatures);
    let serialized = message.serialize();
    let mut signatures = vec![solana_sdk::signature::Signature::default(); signers];
    if sign {
        let keys = &message.account_keys;
        for (index, key) in keys.iter().take(signers).enumerate() {
            if *key == owner.pubkey() {
                signatures[index] = owner.sign_message(&serialized);
            }
        }
    }
    let tx = solana_sdk::transaction::VersionedTransaction {
        signatures,
        message: solana_sdk::message::VersionedMessage::Legacy(message),
    };
    base64::prelude::BASE64_STANDARD.encode(bincode::serialize(&tx).unwrap())
}

/// The mandatory buy-account creation step for the intent.
fn destination_creation(
    funder: solana_sdk::pubkey::Pubkey,
    owner: solana_sdk::pubkey::Pubkey,
    intent: &cow_settlement_interface::data::intent::OrderIntent,
) -> solana_sdk::instruction::Instruction {
    spl_associated_token_account_interface::instruction::create_associated_token_account_idempotent(
        &funder,
        &owner,
        &intent.buy_mint,
        &spl_token_interface::ID,
    )
}

/// A sponsored creation transaction with an arbitrary SPL sell and only the
/// mandatory buy-account creation in front of `CreateOrder`.
fn sponsored_creation_tx(
    funder: solana_sdk::pubkey::Pubkey,
    owner: &solana_sdk::signer::keypair::Keypair,
    sign: bool,
) -> String {
    let intent = sponsored_intent(owner.pubkey(), false);
    let destination = destination_creation(funder, owner.pubkey(), &intent);
    creation_tx(funder, owner, &intent, vec![destination], sign)
}

async fn post_order(addr: SocketAddr, transaction: String) -> (reqwest::StatusCode, String) {
    let response = reqwest::Client::new()
        .post(format!("http://{addr}/api/v1/orders"))
        .json(&serde_json::json!({ "transaction": transaction }))
        .send()
        .await
        .unwrap();
    let status = response.status();
    let json: serde_json::Value = response.json().await.unwrap();
    (
        status,
        json["errorType"].as_str().unwrap_or_default().to_owned(),
    )
}

/// Placement validation runs before any database access: every rejection
/// here answers without a live pool.
#[tokio::test]
async fn create_order_rejects_invalid_submissions() {
    let funder = solana_sdk::pubkey::Pubkey::new_unique();
    let owner = solana_sdk::signer::keypair::Keypair::new();

    // The feature is off without sponsoring config.
    let plain = spawn_server().await;
    let (status, kind) = post_order(plain, "AAAA".to_owned()).await;
    assert_eq!(
        (status, kind.as_str()),
        (reqwest::StatusCode::BAD_REQUEST, "SponsoringDisabled")
    );

    let addr =
        spawn_sponsored_server(PgPool::connect_lazy("postgresql://").unwrap(), funder, true).await;
    for (transaction, expected) in [
        ("bm90IGEgdHg=".to_owned(), "InvalidTransaction"),
        // The owner as fee payer: the funder must front the fees.
        (
            sponsored_creation_tx(owner.pubkey(), &owner, true),
            "WrongFeePayer",
        ),
        // Nobody signed: the owner's signature is required.
        (
            sponsored_creation_tx(funder, &owner, false),
            "InvalidSignature",
        ),
    ] {
        let (status, kind) = post_order(addr, transaction).await;
        assert_eq!(
            (status, kind.as_str()),
            (reqwest::StatusCode::BAD_REQUEST, expected)
        );
    }

    // Intent-level rejections: the funder as owner, equal mints, zero
    // amounts, and a validTo below the minimum validity.
    let mut funder_owned = sponsored_intent(owner.pubkey(), false);
    funder_owned.owner = funder;
    let mut same_token = sponsored_intent(owner.pubkey(), false);
    same_token.buy_mint = same_token.sell_mint;
    let mut zero_amount = sponsored_intent(owner.pubkey(), false);
    zero_amount.sell_amount = 0;
    let mut expiring = sponsored_intent(owner.pubkey(), false);
    expiring.valid_to = u32::try_from(chrono::Utc::now().timestamp() + 30).unwrap();
    for (intent, expected) in [
        (funder_owned, "InvalidTransaction"),
        (same_token, "SameBuyAndSellToken"),
        (zero_amount, "ZeroAmount"),
        (expiring, "InsufficientValidTo"),
    ] {
        let destination = destination_creation(funder, owner.pubkey(), &intent);
        let transaction = creation_tx(funder, &owner, &intent, vec![destination], true);
        let (status, kind) = post_order(addr, transaction).await;
        assert_eq!(
            (status, kind.as_str()),
            (reqwest::StatusCode::BAD_REQUEST, expected)
        );
    }

    // A well-formed transaction whose blockhash already died.
    let addr = spawn_sponsored_server(
        PgPool::connect_lazy("postgresql://").unwrap(),
        funder,
        false,
    )
    .await;
    let (status, kind) = post_order(addr, sponsored_creation_tx(funder, &owner, true)).await;
    assert_eq!(
        (status, kind.as_str()),
        (reqwest::StatusCode::BAD_REQUEST, "BlockhashExpired")
    );
}

/// A message header that leaves the owner outside the signer region is
/// rejected: on chain the creation would demand the owner's signature, so
/// accepting it would only defer the failure past a won auction.
#[tokio::test]
async fn create_order_requires_the_owner_as_signer() {
    let funder = solana_sdk::pubkey::Pubkey::new_unique();
    let owner = solana_sdk::signer::keypair::Keypair::new();
    let addr =
        spawn_sponsored_server(PgPool::connect_lazy("postgresql://").unwrap(), funder, true).await;
    let intent = sponsored_intent(owner.pubkey(), false);
    let destination = destination_creation(funder, owner.pubkey(), &intent);
    let instruction: solana_sdk::instruction::Instruction =
        cow_settlement_client::instruction::CreateOrder {
            program_id: cow_settlement_interface::id(),
            owner: owner.pubkey(),
            created_by: funder,
            intent: &intent,
        }
        .into();
    let mut message = solana_sdk::message::Message::new_with_blockhash(
        &[destination, instruction],
        Some(&funder),
        &solana_sdk::hash::Hash::new_unique(),
    );
    // Demote the owner out of the signer region.
    message.header.num_required_signatures = 1;
    let tx = solana_sdk::transaction::VersionedTransaction {
        signatures: vec![solana_sdk::signature::Signature::default()],
        message: solana_sdk::message::VersionedMessage::Legacy(message),
    };
    let transaction = base64::prelude::BASE64_STANDARD.encode(bincode::serialize(&tx).unwrap());
    let (status, kind) = post_order(addr, transaction).await;
    assert_eq!(
        (status, kind.as_str()),
        (reqwest::StatusCode::BAD_REQUEST, "InvalidSignature")
    );
}

/// Preparation instructions outside the template, touching the funder, or
/// out of order are rejected. All rejections come from validation, so no
/// database or RPC probe is consumed.
#[tokio::test]
async fn create_order_checks_the_preparation_template() {
    let funder = solana_sdk::pubkey::Pubkey::new_unique();
    let owner_keypair = solana_sdk::signer::keypair::Keypair::new();
    let owner = owner_keypair.pubkey();
    let intent = sponsored_intent(owner, true);
    let addr =
        spawn_sponsored_server(PgPool::connect_lazy("postgresql://").unwrap(), funder, true).await;

    let transfer = |from: solana_sdk::pubkey::Pubkey| {
        solana_system_interface::instruction::transfer(&from, &intent.sell_token_account, 1_000)
    };
    let approve = |delegate: solana_sdk::pubkey::Pubkey| {
        spl_token_interface::instruction::approve(
            &spl_token_interface::ID,
            &intent.sell_token_account,
            &delegate,
            &owner,
            &[],
            1_000,
        )
        .unwrap()
    };

    for (preparations, expected) in [
        // The buy-account creation is mandatory.
        (vec![], "InvalidTransaction"),
        // A program outside the template never rides on the funder's fee.
        (
            vec![solana_sdk::instruction::Instruction::new_with_bytes(
                solana_sdk::pubkey::Pubkey::new_unique(),
                &[],
                vec![],
            )],
            "InvalidTransaction",
        ),
        // A transfer draining the funder instead of wrapping the owner's SOL.
        (vec![transfer(funder)], "InvalidTransaction"),
        // A delegation to anyone but the settlement state PDA.
        (
            vec![approve(solana_sdk::pubkey::Pubkey::new_unique())],
            "WrongDelegate",
        ),
        // An account creation for a mint the order does not trade.
        (
            vec![
                spl_associated_token_account_interface::instruction::create_associated_token_account_idempotent(
                    &funder,
                    &owner,
                    &solana_sdk::pubkey::Pubkey::new_unique(),
                    &spl_token_interface::ID,
                ),
            ],
            "InvalidTransaction",
        ),
        // Steps out of template order.
        (
            vec![approve(state_pda()), transfer(owner)],
            "InvalidTransaction",
        ),
        // A step twice.
        (
            vec![approve(state_pda()), approve(state_pda())],
            "InvalidTransaction",
        ),
    ] {
        let transaction = creation_tx(funder, &owner_keypair, &intent, preparations, true);
        let (status, kind) = post_order(addr, transaction).await;
        assert_eq!(
            (status, kind.as_str()),
            (reqwest::StatusCode::BAD_REQUEST, expected)
        );
    }

    // Wrap steps on an order that does not sell native SOL.
    let plain = sponsored_intent(owner, false);
    let wrap =
        solana_system_interface::instruction::transfer(&owner, &plain.sell_token_account, 1_000);
    let transaction = creation_tx(funder, &owner_keypair, &plain, vec![wrap], true);
    let (status, kind) = post_order(addr, transaction).await;
    assert_eq!(
        (status, kind.as_str()),
        (reqwest::StatusCode::BAD_REQUEST, "InvalidTransaction")
    );
}

/// The happy path lands the order and the duplicate is rejected.
#[tokio::test]
#[ignore = "needs the solana.* schema applied to the local database"]
async fn solana_db_create_order_persists_a_sponsored_order() {
    let pool = PgPool::connect("postgresql://").await.unwrap();
    sqlx::query(
        "TRUNCATE solana.order_pda, solana.orders, solana.order_quotes, solana.order_events \
         CASCADE",
    )
    .execute(&pool)
    .await
    .unwrap();
    let funder = solana_sdk::pubkey::Pubkey::new_unique();
    let owner = solana_sdk::signer::keypair::Keypair::new();
    let addr = spawn_sponsored_server(pool.clone(), funder, true).await;
    // The full preparation prefix in front of `CreateOrder`, as the frontend
    // sends it for a first-time native-SOL sell.
    let intent = sponsored_intent(owner.pubkey(), true);
    let preparations = full_preparations(funder, owner.pubkey(), &intent);
    let transaction = creation_tx(funder, &owner, &intent, preparations, true);
    let quote_id = db::save_quote(
        &pool,
        &db::Quote {
            sell_token: ByteArray(intent.sell_mint.to_bytes()),
            buy_token: ByteArray(intent.buy_mint.to_bytes()),
            sell_amount: intent.sell_amount,
            buy_amount: intent.buy_amount,
            kind: OrderKind::Sell,
            solver: ByteArray([0xDD; 32]),
            expiration: chrono::Utc::now() + chrono::Duration::seconds(60),
        },
    )
    .await
    .unwrap();

    let response = reqwest::Client::new()
        .post(format!("http://{addr}/api/v1/orders"))
        .json(&serde_json::json!({ "transaction": transaction.clone(), "quoteId": quote_id }))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), reqwest::StatusCode::CREATED);
    let uid: String = response.json().await.unwrap();
    assert!(uid.starts_with("0x") && uid.len() == 66);

    let (stored, expiry): (Vec<u8>, i64) =
        sqlx::query_as("SELECT presigned_transaction, last_valid_block_height FROM solana.orders")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert!(!stored.is_empty());
    // Mocked height 100 plus the maximum blockhash age.
    assert_eq!(expiry, 250);
    let linked: (Vec<u8>, Option<i64>, String) =
        sqlx::query_as("SELECT order_uid, quote_id, sell_amount::text FROM solana.order_quotes")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(linked.0, const_hex::decode(&uid[2..]).unwrap());
    assert_eq!(linked.1, Some(quote_id));
    assert_eq!(linked.2, intent.sell_amount.to_string());

    // The duplicate check runs after the RPC probes and the mock answers
    // each probe once, so the duplicate goes through a fresh server over the
    // same database.
    let addr = spawn_sponsored_server(pool.clone(), funder, true).await;
    let (status, kind) = post_order(addr, transaction).await;
    assert_eq!(
        (status, kind.as_str()),
        (reqwest::StatusCode::BAD_REQUEST, "DuplicatedOrder")
    );

    // A named quote that does not match the order (wrong pair here) is
    // dropped: the order lands unlinked.
    let addr = spawn_sponsored_server(pool.clone(), funder, true).await;
    let other_owner = solana_sdk::signer::keypair::Keypair::new();
    let other = sponsored_intent(other_owner.pubkey(), false);
    let mismatched = db::save_quote(
        &pool,
        &db::Quote {
            sell_token: ByteArray([0x99; 32]),
            buy_token: ByteArray(other.buy_mint.to_bytes()),
            sell_amount: other.sell_amount,
            buy_amount: other.buy_amount,
            kind: OrderKind::Sell,
            solver: ByteArray([0xDD; 32]),
            expiration: chrono::Utc::now() + chrono::Duration::seconds(60),
        },
    )
    .await
    .unwrap();
    let destination = destination_creation(funder, other_owner.pubkey(), &other);
    let transaction = creation_tx(funder, &other_owner, &other, vec![destination], true);
    let response = reqwest::Client::new()
        .post(format!("http://{addr}/api/v1/orders"))
        .json(&serde_json::json!({ "transaction": transaction, "quoteId": mismatched }))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), reqwest::StatusCode::CREATED);
    let copies: i64 = sqlx::query_scalar("SELECT count(*) FROM solana.order_quotes")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(copies, 1, "the mismatched quote must not be copied");

    // An expired quote is dropped even when everything else matches.
    let addr = spawn_sponsored_server(pool.clone(), funder, true).await;
    let late_owner = solana_sdk::signer::keypair::Keypair::new();
    let late = sponsored_intent(late_owner.pubkey(), false);
    let expired = db::save_quote(
        &pool,
        &db::Quote {
            sell_token: ByteArray(late.sell_mint.to_bytes()),
            buy_token: ByteArray(late.buy_mint.to_bytes()),
            sell_amount: late.sell_amount,
            buy_amount: late.buy_amount,
            kind: OrderKind::Sell,
            solver: ByteArray([0xDD; 32]),
            expiration: chrono::Utc::now() - chrono::Duration::seconds(1),
        },
    )
    .await
    .unwrap();
    let destination = destination_creation(funder, late_owner.pubkey(), &late);
    let transaction = creation_tx(funder, &late_owner, &late, vec![destination], true);
    let response = reqwest::Client::new()
        .post(format!("http://{addr}/api/v1/orders"))
        .json(&serde_json::json!({ "transaction": transaction, "quoteId": expired }))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), reqwest::StatusCode::CREATED);
    let copies: i64 = sqlx::query_scalar("SELECT count(*) FROM solana.order_quotes")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(copies, 1, "the expired quote must not be copied");
}
